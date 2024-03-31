use std::collections::HashMap;
use serde::{ Serialize, Deserialize };

use crate::discord::DiscordMember;

pub mod stream;
pub use stream::ElementStream;

#[derive(Clone, Debug, Deserialize)]
pub struct Document {
	//pub id: Uuid,
	pub name: String,
	pub kind: DocumentKind,
	pub active: bool,
	pub definition: Vec<Element>
}

impl Document {
	pub fn into_stream(self, variables: Variable) -> (ElementStream, ActionTracker) {
		(ElementStream::new(self.definition, variables), ActionTracker::new())
	}

	pub fn is_ready_for_stream(&self) -> bool {
		return self.active && !self.definition.is_empty();
	}
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
	//pub id: Uuid,
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
	//pub text: String
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

	#[serde(rename = "iterable.has_any_value")]
	HasAnyValue,
	#[serde(rename = "iterable.does_not_have_any_value")]
	DoesNotHaveAnyValue,
	#[serde(rename = "iterable.contains")]
	Contains,
	#[serde(rename = "iterable.does_not_contain")]
	DoesNotContain,
	#[serde(rename = "iterable.begins_with")]
	BeginsWith,
	#[serde(rename = "iterable.ends_with")]
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
				serde_json::Value::String(x) => VariableKind::String(x.clone()).into(),
				_ => unimplemented!()
			}),
			StatementInput::Variable(reference) => reference.resolve(&root_variable)
		}
	}
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct Variable {
	pub kind: VariableKind,
	pub interpret_as: VariableInterpretAs
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum VariableKind {
	Map(HashMap<String, Variable>),
	List(Vec<Variable>),
	String(String)
}

impl Into<Variable> for VariableKind {
	fn into(self) -> Variable {
		Variable {
			kind: self,
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum VariableInterpretAs {
	NonSpecific,
	Member,
	Message
}

impl Variable {
	pub fn create_map<const N: usize>(value: [(&str, Self); N], interpret_as: Option<VariableInterpretAs>) -> Self {
		Self {
			kind: VariableKind::Map(value.into_iter().map(|x| (x.0.to_string(), x.1)).collect()),
			interpret_as: interpret_as.unwrap_or(VariableInterpretAs::NonSpecific)
		}
	}

	pub fn insert(&mut self, key: impl Into<String>, variable: Variable) {
		match &mut self.kind {
			VariableKind::Map(x) => x.insert(key.into(), variable),
			_ => panic!()
		};
	}

	pub fn get(&self, key: &str) -> &Variable {
		self.as_map().unwrap().get(key).unwrap()
	}

	pub fn as_map(&self) -> Option<&HashMap<String, Variable>> {
		match &self.kind {
			VariableKind::Map(x) => Some(x),
			_ => None
		}
	}

	pub fn cast_str(&self) -> &str {
		match &self.kind {
			VariableKind::String(x) => x,
			_ => panic!()
		}
	}

	pub fn is_empty(&self) -> bool {
		match &self.kind {
			VariableKind::Map(x) => x.is_empty(),
			VariableKind::List(x) => x.is_empty(),
			VariableKind::String(x) => x.is_empty()
		}
	}

	pub fn contains(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::Map(x) => x.iter().any(|x| x.1 == variable),
			VariableKind::List(x) => x.iter().any(|x| x == variable),
			VariableKind::String(x) => x.contains(variable.cast_str())
		}
	}

	pub fn starts_with(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::Map(_) => false,
			VariableKind::List(x) => x.first().is_some_and(|x| x == variable),
			VariableKind::String(x) => x.starts_with(variable.cast_str())
		}
	}

	pub fn ends_with(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::Map(_) => false,
			VariableKind::List(x) => x.last().is_some_and(|x| x == variable),
			VariableKind::String(x) => x.ends_with(variable.cast_str())
		}
	}
}

impl Into<Variable> for String {
	fn into(self) -> Variable {
		Variable {
			kind: VariableKind::String(self),
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

impl Into<Variable> for DiscordMember {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", VariableKind::String(self.id()).into()),
			("roles", VariableKind::List(self.roles.iter().map(|x| x.clone().into()).collect()).into()),
			("guild_id", VariableKind::String(self.guild_id.clone()).into()),
			("username", VariableKind::String(self.user.username.clone()).into()),
			("avatar_url", VariableKind::String(self.user.avatar_url().unwrap_or("".into())).into()),
			("display_name", VariableKind::String(self.display_name()).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl Into<Variable> for twilight_model::user::User {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", VariableKind::String(self.id.to_string()).into()),
			("username", VariableKind::String(self.name.clone()).into()),
			("avatar_url", VariableKind::String(self.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.id)).unwrap_or("".into())).into()),
			("display_name", VariableKind::String(self.global_name.unwrap_or(self.name)).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl Into<Variable> for &twilight_model::gateway::payload::incoming::MessageCreate {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", VariableKind::String(self.id.to_string()).into()),
			("content", VariableKind::String(self.content.clone()).into()),
			("channel_id", VariableKind::String(self.channel_id.to_string()).into())
		], Some(VariableInterpretAs::Message))
	}
}

impl Into<Variable> for &twilight_model::gateway::payload::incoming::MemberUpdate {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", VariableKind::String(self.user.id.to_string()).into()),
			("roles", VariableKind::List(self.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", VariableKind::String(self.guild_id.to_string()).into()),
			("username", VariableKind::String(self.user.name.clone()).into()),
			("avatar_url", VariableKind::String(self.avatar.or(self.user.avatar).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.user.id)).unwrap_or("".into())).into()),
			("display_name", VariableKind::String(self.user.global_name.clone().unwrap_or_else(|| self.user.name.clone())).into())
		], Some(VariableInterpretAs::Member))
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
			variable = Some(match &match variable {
				Some(x) => x,
				_ => root_variable
			}.kind {
				VariableKind::Map(map) => match map.get(key) {
					Some(x) => x,
					_ => return None
				},
				_ => return None
			});
		}

		variable.cloned()
	}
}

pub struct ActionTracker {
	items: Vec<ActionTrackerItem>
}

impl ActionTracker {
	pub fn new() -> Self {
		Self {
			items: vec![]
		}
	}

	pub fn assigned_member_role(&mut self, user_id: impl ToString, role_id: impl ToString) {
		self.items.push(ActionTrackerItem::AssignedMemberRole(user_id.to_string(), role_id.to_string()));
	}
}

pub enum ActionTrackerItem {
	AssignedMemberRole(String, String)
}