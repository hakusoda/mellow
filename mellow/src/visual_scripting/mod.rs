use std::collections::HashMap;
use uuid::Uuid;
use serde::{ Serialize, Deserialize };

use crate::discord::DiscordMember;

pub mod stream;
pub use stream::ElementStream;

#[derive(Clone, Debug, Deserialize)]
pub struct Document {
	//pub id: Uuid,
	pub name: String,
	pub kind: DocumentKind,
	pub definition: Vec<Element>
}


#[derive(Eq, Hash, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DocumentKind {
	#[serde(rename = "mellow.discord_event.member_join")]
	MemberJoinEvent,
	#[serde(rename = "mellow.discord_event.message_create")]
	MessageCreatedEvent,
	#[serde(rename = "mellow.discord_event.member.completed_onboarding")]
	MemberCompletedOnboardingEvent
}

impl ToString for DocumentKind {
	fn to_string(&self) -> String {
		// how silly is this? how silly? AHHHHHHHhhhhhh
		let string = serde_json::to_string(self).unwrap();
		let chars = string.chars().skip(1);
		chars.clone().take(chars.count() - 1).collect()
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct Element {
	pub id: Uuid,
	#[serde(flatten)]
	pub kind: ElementKind
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum ElementKind {
	#[serde(rename = "action.mellow.member.ban")]
	BanMember(VariableReference),
	#[serde(rename = "action.mellow.member.kick")]
	KickMember(VariableReference),
	#[serde(rename = "action.mellow.member.sync")]
	SyncMember,

	#[serde(rename = "action.mellow.member.roles.assign")]
	AssignRoleToMember(StringValueWithVariableReference),

	#[serde(rename = "action.mellow.message.reply")]
	Reply(StringValueWithVariableReference),
	#[serde(rename = "action.mellow.message.reaction.create")]
	AddReaction(StringValueWithVariableReference),

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
pub struct StringValueWithVariableReference {
	pub value: String,
	pub reference: VariableReference
}

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
	pub text: String
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
	Variable(VariableReference)
}

impl StatementInput {
	fn resolve(&self, root_variable: &Variable) -> Option<Variable> {
		match self {
			StatementInput::Match(value) => Some(match value {
				serde_json::Value::String(x) => Variable::String(x.clone()),
				_ => unimplemented!()
			}),
			StatementInput::Variable(reference) => reference.resolve(&root_variable)
		}
	}
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum Variable {
	Map(HashMap<String, Variable>),
	String(String),
	Member(MemberVariable),
	Message(MessageVariable)
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct MemberVariable {
	pub id: String,
	pub username: String,
	pub avatar_url: Option<String>,
	pub display_name: String
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct MessageVariable {
	pub id: String,
	pub content: String,
	pub channel_id: String
}

impl Variable {
	pub fn create_map<const N: usize>(value: [(&str, Self); N]) -> Self {
		Self::Map(value.into_iter().map(|x| (x.0.to_string(), x.1)).collect())
	}

	pub fn insert(&mut self, key: impl Into<String>, variable: Variable) {
		match self {
			Variable::Map(x) => x.insert(key.into(), variable),
			_ => panic!()
		};
	}

	pub fn cast_str(&self) -> &str {
		match self {
			Variable::String(x) => x,
			_ => panic!()
		}
	}
}

impl Into<Variable> for DiscordMember {
	fn into(self) -> Variable {
		Variable::Member(MemberVariable {
			id: self.id(),
			username: self.user.username.clone(),
			avatar_url: self.user.avatar_url(),
			display_name: self.display_name()
		})
	}
}

impl Into<Variable> for twilight_model::user::User {
	fn into(self) -> Variable {
		Variable::Member(MemberVariable {
			id: self.id.to_string(),
			username: self.name.clone(),
			avatar_url: self.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.id)),
			display_name: self.global_name.unwrap_or(self.name)
		})
	}
}

impl Into<Variable> for &twilight_model::gateway::payload::incoming::MessageCreate {
	fn into(self) -> Variable {
		Variable::Message(MessageVariable {
			id: self.id.to_string(),
			content: self.content.clone(),
			channel_id: self.channel_id.to_string()
		})
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct VariableReference {
	path: String
}

impl VariableReference {
	pub fn resolve(&self, root_variable: &Variable) -> Option<Variable> {
		let mut variable: Option<&Variable> = None;
		for key in self.path.split("::") {
			variable = Some(match match variable {
				Some(x) => x,
				_ => root_variable
			} {
				Variable::Map(map) => match map.get(key) {
					Some(x) => x,
					_ => return None
				}
				_ => return None
			});
		}

		variable.cloned()
	}
}