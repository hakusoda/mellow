use std::collections::HashMap;
use serde::Deserialize;

pub mod stream;
pub use stream::ElementStream;

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