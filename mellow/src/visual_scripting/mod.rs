use std::collections::HashMap;
use serde::Deserialize;
use futures::stream::Stream;

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum Element {
	#[serde(rename = "action.mellow.sync_profile")]
	SyncMemberProfile,
	#[serde(rename = "action.mellow.member.ban")]
	BanMember,
	#[serde(rename = "action.mellow.member.kick")]
	KickMember,

	#[serde(rename = "no_op.comment")]
	Comment,
	#[serde(rename = "no_op.nothing")]
	Nothing,

	#[serde(rename = "special.root")]
	Root,

	#[serde(rename = "statement.if")]
	IfStatement(ConditionalStatement)
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConditionalStatement {
	pub blocks: Vec<ConditionalStatementBlock>
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConditionalStatementBlock {
	pub items: Vec<Element>,
	pub inputs: Vec<StatementInput>,
	pub condition: Option<StatementCondition>
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum StatementCondition {
	#[serde(rename = "generic.is")]
	Is,
	#[serde(rename = "generic.is_not")]
	IsNot,
	#[serde(rename = "generic.contains")]
	Contains,
	#[serde(rename = "generic.does_not_contain")]
	DoesNotContain,
	#[serde(rename = "string.starts_with")]
	StartsWith,
	#[serde(rename = "string.ends_with")]
	EndsWith
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StatementInput {
	Match(serde_json::Value),
	Variable(String)
}

impl StatementInput {
	fn resolve(&self, variables: &HashMap<String, serde_json::Value>) -> Option<serde_json::Value> {
		match self {
			StatementInput::Match(value) => Some(value.clone()),
			StatementInput::Variable(path) => {
				let mut value: Option<&serde_json::Value> = None;
				for key in path.split("::") {
					if let Some(val) = value {
						value = val.get(key);
					} else {
						value = variables.get(key);
					}
				}
	
				value.cloned()
			}
		}
	}
}

pub struct ElementStream {
	// would something else be better-suited for this?
	iterator: Box<dyn Iterator<Item = Element> + Send>,
	variables: HashMap<String, serde_json::Value>,
	current_sub_stream: Option<Box<ElementStream>>,
	current_statement_stream: Option<StatementStream>
}

impl ElementStream {
	pub fn new(elements: Vec<Element>, variables: HashMap<String, serde_json::Value>) -> Self {
		Self {
			iterator: Box::new(elements.into_iter()),
			variables,
			current_sub_stream: None,
			current_statement_stream: None
		}
	}

	fn get_next(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Element>> {
		if self.current_sub_stream.is_some() {
			self.poll_sub_stream(cx)
		} else {
			match &mut self.current_statement_stream {
				Some(statement_stream) => {
					match statement_stream.get_next(cx) {
						std::task::Poll::Ready(x) => match x {
							Some(x) => {
								self.current_sub_stream = Some(Box::new(ElementStream {
									iterator: Box::new(x.items.into_iter()),
									variables: self.variables.clone(),
									current_sub_stream: None,
									current_statement_stream: None
								}));
								return self.poll_sub_stream(cx);
							},
							None => self.current_statement_stream = None
						},
						_ => ()
					}
					self.get_next(cx)
				},
				None => if let Some(item) = self.iterator.next() {
					match item {
						Element::IfStatement(statement) => {
							self.current_statement_stream = Some(StatementStream {
								iterator: Box::new(statement.blocks.into_iter()),
								variables: self.variables.clone()
							});
							self.poll_statement_stream(cx)
						},
						_ => std::task::Poll::Ready(Some(item))
					}
				} else { std::task::Poll::Ready(None) }
			}
		}
	}

	fn poll_sub_stream(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Element>> {
		// polls the sub stream for a potential element, otherwise, the current stream will continue iterating.
		match self.current_sub_stream.as_mut().unwrap().get_next(cx) {
			std::task::Poll::Ready(x) => match x {
				Some(x) => return std::task::Poll::Ready(Some(x)),
				// removes the sub stream, as it is now empty.
				None => self.current_sub_stream = None
			},
			_ => ()
		}
		self.get_next(cx)
	}

	fn poll_statement_stream(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Element>> {
		match self.current_statement_stream.as_mut().unwrap().get_next(cx) {
			std::task::Poll::Ready(x) => match x {
				// creates a sub element stream iterating over the statement block's containing items.
				Some(x) => self.current_sub_stream = Some(Box::new(ElementStream {
					iterator: Box::new(x.items.into_iter()),
					variables: self.variables.clone(),
					current_sub_stream: None,
					current_statement_stream: None
				})),
				// removes the statement stream, as it is now empty.
				None => self.current_statement_stream = None
			},
			_ => ()
		}
		self.get_next(cx)
	}
}

impl Stream for ElementStream {
	type Item = Element;
	fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
		self.get_mut().get_next(cx)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iterator.size_hint()
	}
}

struct StatementStream {
	iterator: Box<dyn Iterator<Item = ConditionalStatementBlock> + Send>,
	variables: HashMap<String, serde_json::Value>
}

impl StatementStream {
	fn get_next(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<ConditionalStatementBlock>> {
		if let Some(block) = self.iterator.next() {
			if let Some(condition) = &block.condition {
				let variables = &self.variables;

				// TODO: return an error if the inputs can't be resolved, said error should be logged to the server if possible.
				if let Some(input_a) = block.inputs.first().and_then(|x| x.resolve(&variables)) {
					if let Some(input_b) = block.inputs.get(1).and_then(|x| x.resolve(&variables))  {
						if !match condition {
							StatementCondition::Is => input_a == input_b,
							StatementCondition::IsNot => input_a != input_b,
							StatementCondition::Contains => input_a.as_str().unwrap().contains(input_b.as_str().unwrap()),
							StatementCondition::DoesNotContain => input_a.as_str().unwrap().contains(input_b.as_str().unwrap()),
							StatementCondition::StartsWith => input_a.as_str().unwrap().starts_with(input_b.as_str().unwrap()),
							StatementCondition::EndsWith => input_a.as_str().unwrap().ends_with(input_b.as_str().unwrap())
						} {
							return self.get_next(cx);
						}
					}
				}
			}
			std::task::Poll::Ready(Some(block))
		} else { std::task::Poll::Ready(None) }
	}
}

impl Stream for StatementStream {
	type Item = ConditionalStatementBlock;
	fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
		self.get_mut().get_next(cx)
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iterator.size_hint()
	}
}