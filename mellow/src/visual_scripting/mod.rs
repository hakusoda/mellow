use std::collections::HashMap;
use serde::{ Serialize, Deserialize };
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	server::{
		logging::ServerLog,
		Server
	},
	discord::GuildMember,
	Result
};

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
		(ElementStream::new(self.definition, variables), ActionTracker::new(self.name))
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
	#[serde(rename = "action.mellow.message.delete")]
	DeleteMessage(VariableReference),

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
	pub blocks: Vec<StatementBlock>
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatementBlock {
	pub items: Vec<Element>,
	pub conditions: Vec<StatementCondition>
}

#[derive(Clone, Debug, Deserialize)]
pub struct StatementCondition {
	pub kind: StatementConditionKind,
	pub inputs: Vec<StatementInput>,
	pub condition: Condition
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementConditionKind {
	Initial,
	And,
	Or
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum Condition {
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
	#[serde(rename = "iterable.contains_only")]
	ContainsOnly,
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
			StatementInput::Match(value) => Some(value.into()),
			StatementInput::Variable(reference) => reference.resolve(&root_variable)
		}
	}
}

impl Into<Variable> for &serde_json::Value {
	fn into(self) -> Variable {
		match self {
			serde_json::Value::Array(x) => VariableKind::List(x.iter().map(|x| x.into()).collect()),
			serde_json::Value::String(x) => VariableKind::String(x.clone()),
			_ => unimplemented!()
		}.into()
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

	pub fn contains_only(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::Map(_) => false,
			VariableKind::List(x) => match &variable.kind {
				VariableKind::Map(_) => false,
				VariableKind::List(y) => x.iter().all(|x| y.iter().any(|y| x == y)),
				VariableKind::String(_) => false
			},
			VariableKind::String(_) => false
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

impl<T> Into<Variable> for &Id<T> {
	fn into(self) -> Variable {
		self.to_string().into()
	}
}

impl GuildMember {
	pub fn into_variable(&self, server_id: &Id<GuildMarker>) -> Variable {
		Variable::create_map([
			("id", self.id().into()),
			("roles", VariableKind::List(self.roles.iter().map(|x| x.clone().into()).collect()).into()),
			("guild_id", server_id.to_string().into()),
			("username", self.user.username.clone().into()),
			("avatar_url", self.user.avatar_url().unwrap_or("".into()).into()),
			("display_name", self.display_name().into())
		], Some(VariableInterpretAs::Member))
	}
}

impl Into<Variable> for twilight_model::user::User {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", self.id.to_string().into()),
			("username", self.name.clone().into()),
			("avatar_url", self.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.id)).unwrap_or("".into()).into()),
			("display_name", self.global_name.unwrap_or(self.name).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl Variable {
	pub fn from_partial_member(user: &twilight_model::user::User, member: &twilight_model::guild::PartialMember, guild_id: &Id<GuildMarker>) -> Variable {
		Variable::create_map([
			("id", user.id.to_string().into()),
			("roles", VariableKind::List(member.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", guild_id.to_string().into()),
			("username", user.name.clone().into()),
			("avatar_url", member.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", user.id)).unwrap_or("".into()).into()),
			("display_name", user.global_name.clone().unwrap_or_else(|| user.name.clone()).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl Into<Variable> for &twilight_model::gateway::payload::incoming::MessageCreate {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", self.id.to_string().into()),
			("author", self.author.clone().into()),
			("content", self.content.clone().into()),
			("channel_id", self.channel_id.to_string().into())
		], Some(VariableInterpretAs::Message))
	}
}

impl Into<Variable> for &twilight_model::gateway::payload::incoming::MemberUpdate {
	fn into(self) -> Variable {
		Variable::create_map([
			("id", self.user.id.to_string().into()),
			("roles", VariableKind::List(self.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", self.guild_id.to_string().into()),
			("username", self.user.name.clone().into()),
			("avatar_url", self.avatar.or(self.user.avatar).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.user.id)).unwrap_or("".into()).into()),
			("display_name", self.user.global_name.clone().unwrap_or_else(|| self.user.name.clone()).into())
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
	items: Vec<ActionTrackerItem>,
	document_name: String
}

impl ActionTracker {
	pub fn new(document_name: String) -> Self {
		Self {
			items: vec![],
			document_name
		}
	}

	pub async fn send_logs(self, guild_id: &Id<GuildMarker>) -> Result<()> {
		if !self.items.is_empty() {
			let server = Server::fetch(guild_id.to_string()).await?;
			server.send_logs(vec![ServerLog::VisualScriptingDocumentResult {
				items: self.items,
				document_name: self.document_name
			}]).await?;
		}
		Ok(())
	}

	pub fn assigned_member_role(&mut self, user_id: impl ToString, role_id: impl ToString) {
		self.items.push(ActionTrackerItem::AssignedMemberRole(user_id.to_string(), role_id.to_string()));
	}

	pub fn banned_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::BannedMember(user_id.to_string()));
	}

	pub fn kicked_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::KickedMember(user_id.to_string()));
	}

	pub fn deleted_message(&mut self, channel_id: impl ToString, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::DeletedMessage(channel_id.to_string(), user_id.to_string()));
	}
}

#[derive(Serialize, Deserialize)]
pub enum ActionTrackerItem {
	AssignedMemberRole(String, String),
	BannedMember(String),
	KickedMember(String),
	DeletedMessage(String, String)
}