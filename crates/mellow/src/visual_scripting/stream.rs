use std::sync::Arc;
use tokio::sync::RwLock;
use futures::Stream;

use super::{ Element, Variable, Condition, ElementKind, StatementBlock, StatementConditionKind };

pub struct ElementStream {
	// would something else be better-suited for this?
	iterator: Box<dyn Iterator<Item = Element> + Send>,
	variables: Arc<RwLock<Variable>>,
	current_sub_stream: Option<Box<ElementStream>>,
	current_statement_stream: Option<StatementStream>
}

impl ElementStream {
	pub fn new(elements: Vec<Element>, variables: Variable) -> Self {
		Self {
			iterator: Box::new(elements.into_iter()),
			variables: Arc::new(RwLock::new(variables)),
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
					if let std::task::Poll::Ready(result) = statement_stream.get_next() {
						match result {
							Some(x) => {
								self.current_sub_stream = Some(Box::new(ElementStream {
									iterator: Box::new(x.items.into_iter()),
									variables: Arc::new(RwLock::new(self.variables.try_read().unwrap().clone())),
									current_sub_stream: None,
									current_statement_stream: None
								}));
								return self.poll_sub_stream(cx);
							},
							None => self.current_statement_stream = None
						}
					}
					self.get_next(cx)
				},
				None => if let Some(item) = self.iterator.next() {
					match item.kind {
						ElementKind::IfStatement(statement) => {
							self.current_statement_stream = Some(StatementStream {
								iterator: Box::new(statement.blocks.into_iter()),
								variables: self.variables.try_read().unwrap().clone()
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
		if let std::task::Poll::Ready(result) = self.current_sub_stream.as_mut().unwrap().get_next(cx) {
			match result {
				Some(x) => return std::task::Poll::Ready(Some(x)),
				// removes the sub stream, as it is now empty.
				None => self.current_sub_stream = None
			}
		}
		self.get_next(cx)
	}

	fn poll_statement_stream(&mut self, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Element>> {
		if let std::task::Poll::Ready(result) = self.current_statement_stream.as_mut().unwrap().get_next() {
			match result {
				// creates a sub element stream iterating over the statement block's containing items.
				Some(x) => self.current_sub_stream = Some(Box::new(ElementStream {
					iterator: Box::new(x.items.into_iter()),
					variables: self.variables.clone(),
					current_sub_stream: None,
					current_statement_stream: None
				})),
				// removes the statement stream, as it is now empty.
				None => self.current_statement_stream = None
			}
		}
		self.get_next(cx)
	}
}

impl Stream for ElementStream {
	type Item = (Element, Arc<RwLock<Variable>>);
	fn poll_next(self: std::pin::Pin<&mut Self>, cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
		let stream = self.get_mut();
		stream.get_next(cx).map(move |x| x.map(|x| (x, stream.variables.clone())))
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iterator.size_hint()
	}
}

pub struct StatementStream {
	iterator: Box<dyn Iterator<Item = StatementBlock> + Send>,
	variables: Variable
}

impl StatementStream {
	fn get_next(&mut self) -> std::task::Poll<Option<StatementBlock>> {
		if let Some(block) = self.iterator.next() {
			let mut last_value = false;
			for condition in block.conditions.iter() {
				let variables = &self.variables;

				// TODO: return an error if the inputs can't be resolved, said error should be logged to the server if possible.
				let input_a = condition.inputs.first().and_then(|x| x.resolve(variables));
				let input_b = condition.inputs.get(1).and_then(|x| x.resolve(variables));
				let value = match condition.condition {
					Condition::Is => input_a.is_some() && input_a == input_b,
					Condition::IsNot => input_a.is_some() && input_a != input_b,
					Condition::HasAnyValue => input_a.map_or(false, |x| !x.is_empty()),
					Condition::DoesNotHaveAnyValue => input_a.map_or(false, |x| x.is_empty()),
					Condition::Contains => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						input_a.contains(&input_b)
					} else { false },
					Condition::ContainsOnly => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						input_a.contains_only(&input_b)
					} else { false },
					Condition::ContainsOneOf => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						input_a.contains_one_of(&input_b)
					} else { false },
					Condition::DoesNotContain => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						!input_a.contains(&input_b)
					} else { false },
					Condition::DoesNotContainOneOf => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						!input_a.contains_one_of(&input_b)
					} else { false },
					Condition::BeginsWith => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						input_a.starts_with(&input_b)
					} else { false },
					Condition::EndsWith => if let Some(input_a) = input_a && let Some(input_b) = input_b {
						input_a.ends_with(&input_b)
					} else { false }
				};
				match condition.kind {
					StatementConditionKind::Initial => (),
					StatementConditionKind::And => if !last_value {
						break;
					},
					StatementConditionKind::Or => if last_value {
						break;
					}
				}
				last_value = value;
			}
			if last_value {
				loop {
					if self.iterator.next().is_none() {
						break;
					}
				}
				std::task::Poll::Ready(Some(block))
			} else { self.get_next() }
		} else { std::task::Poll::Ready(None) }
	}
}

impl Stream for StatementStream {
	type Item = StatementBlock;
	fn poll_next(self: std::pin::Pin<&mut Self>, _cx: &mut std::task::Context<'_>) -> std::task::Poll<Option<Self::Item>> {
		self.get_mut().get_next()
	}

	fn size_hint(&self) -> (usize, Option<usize>) {
		self.iterator.size_hint()
	}
}