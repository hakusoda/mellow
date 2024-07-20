use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::DocumentMarker,
		HakuId
	},
	PG_POOL
};
use serde::{ Serialize, Deserialize };
use std::{
	fmt::Display,
	pin::Pin
};

use crate::Result;
use variable::VariableReference;

pub mod variable;
pub use variable::{ Variable, VariableKind };

#[derive(Clone, Debug, Deserialize)]
pub struct DocumentModel {
	pub id: HakuId<DocumentMarker>,
	pub name: String,
	pub kind: DocumentKind,
	pub active: bool,
	pub definition: Vec<Element>
}

impl DocumentModel {
	pub async fn get(document_id: HakuId<DocumentMarker>) -> Result<Option<Self>> {
		Self::get_many(&[document_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(document_ids: &[HakuId<DocumentMarker>]) -> Result<Vec<Self>> {
		let document_ids: Vec<_> = document_ids
			.iter()
			.map(|x| x.value)
			.collect();
		Ok(sqlx::query!(
			"
			SELECT id, name, kind, active, definition
			FROM visual_scripting_documents
			WHERE id = ANY($1)
			",
			&document_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(Self {
					id: record.id.into(),
					name: record.name,
					kind: serde_json::from_str(&format!("\"{}\"", record.kind)).unwrap(),
					active: record.active,
					definition: serde_json::from_value(record.definition).unwrap()
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub fn clone_if_ready(&self) -> Option<Self> {
		if self.active && !self.definition.is_empty() {
			Some(self.clone())
		} else { None }
	}
}


#[derive(Eq, Hash, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum DocumentKind {
	#[serde(rename = "mellow.command")]
	MellowCommand,

	#[serde(rename = "mellow.discord_event.member_join")]
	MemberJoinEvent,
	#[serde(rename = "mellow.discord_event.message_create")]
	MessageCreatedEvent,
	#[serde(rename = "mellow.discord_event.member.updated")]
	MemberUpdatedEvent,
	#[serde(rename = "mellow.discord_event.member.completed_onboarding")]
	MemberCompletedOnboardingEvent,

	#[serde(rename = "mellow.event.member.synced")]
	MemberSynced
}

impl Display for DocumentKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		// how silly is this? how silly? AHHHHHHHhhhhhh
		let string = simd_json::to_string(self).unwrap();
		let chars = string.chars().skip(1);
		write!(f, "{}", chars.clone().take(chars.count() - 1).collect::<String>())
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Element {
	//pub id: Uuid,
	#[serde(flatten)]
	pub kind: ElementKind
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
	#[serde(rename = "action.mellow.member.roles.remove")]
	RemoveRoleFromMember(StringValueWithVariableReference),

	#[serde(rename = "action.mellow.message.reply")]
	Reply(StringValueWithVariableReference),
	#[serde(rename = "action.mellow.message.reaction.create")]
	AddReaction(StringValueWithVariableReference),

	#[serde(rename = "action.mellow.message.create")]
	CreateMessage(Message),
	#[serde(rename = "action.mellow.message.delete")]
	DeleteMessage(VariableReference),

	#[serde(rename = "action.mellow.message.start_thread")]
	StartThreadFromMessage {
		name: Text,
		message: VariableReference
	},

	#[serde(rename = "action.mellow.interaction.reply")]
	InteractionReply(Text),

	#[serde(rename = "get_data.mellow.server.current_patreon_campaign")]
	GetLinkedPatreonCampaign,

	#[serde(rename = "no_op.comment")]
	Comment,
	#[serde(rename = "no_op.nothing")]
	Nothing,

	#[serde(rename = "special.root")]
	Root,

	#[serde(rename = "statement.if")]
	IfStatement(ConditionalStatement)
}

impl ElementKind {
	pub fn display_name(&self) -> &str {
		match self {
			ElementKind::AddReaction(_) => "Add reaction to message",
			ElementKind::AssignRoleToMember(_) => "Assign role to member",
			ElementKind::RemoveRoleFromMember(_) => "Remove role from member",
			ElementKind::BanMember(_) => "Ban member from the server",
			ElementKind::Comment => "Comment",
			ElementKind::CreateMessage(_) => "Send message in channel",
			ElementKind::DeleteMessage(_) => "Delete message",
			ElementKind::StartThreadFromMessage { .. } => "Start thread from message",
			ElementKind::GetLinkedPatreonCampaign => "Get linked patreon campaign",
			ElementKind::IfStatement(_) => "If",
			ElementKind::InteractionReply(_) => "Reply to author",
			ElementKind::KickMember(_) => "Kick member from the server",
			ElementKind::Nothing => "Nothing",
			ElementKind::Reply(_) => "Reply to message",
			ElementKind::Root => "Root",
			ElementKind::SyncMember => "Sync member's profile"
		}
	}
}

impl Display for ElementKind {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		let string = simd_json::to_string(self).unwrap();
		let mut kind = String::new();
		let mut quot_count: i32 = 0;
		for char in string.chars() {
			if char == '"' {
				quot_count += 1;
				if quot_count > 3 {
					break;
				}
			} else if quot_count > 2 {
				kind += &char.to_string();
			}
		}
		write!(f, "{kind}")
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StringValueWithVariableReference {
	pub value: String,
	pub reference: VariableReference
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Message {
	pub content: Text,
	pub channel_id: StatementInput
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Text {
	pub value: Vec<TextElement>
}

impl Text {
	pub fn resolve(&self, root_variable: &Variable) -> String {
		self.value.iter().map(|x| match x {
			TextElement::String(x) => x.clone(),
			TextElement::Variable(x) => x.resolve(root_variable).unwrap().cast_string()
		}).collect::<Vec<String>>().join("")
	}
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextElement {
	String(String),
	Variable(VariableReference)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ConditionalStatement {
	pub blocks: Vec<StatementBlock>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatementBlock {
	pub items: Vec<Element>,
	pub conditions: Vec<StatementCondition>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct StatementCondition {
	pub kind: StatementConditionKind,
	pub inputs: Vec<StatementInput>,
	pub condition: Condition
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StatementConditionKind {
	Initial,
	And,
	Or
}

#[derive(Clone, Debug, Serialize, Deserialize)]
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
	#[serde(rename = "iterable.contains_one_of")]
	ContainsOneOf,
	#[serde(rename = "iterable.does_not_contain")]
	DoesNotContain,
	#[serde(rename = "iterable.does_not_contain_one_of")]
	DoesNotContainOneOf,
	#[serde(rename = "iterable.begins_with")]
	BeginsWith,
	#[serde(rename = "iterable.ends_with")]
	EndsWith
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum StatementInput {
	Match(serde_json::Value),
	Variable(VariableReference)
}

impl StatementInput {
	pub fn resolve(&self, root_variable: &Variable) -> Option<Variable> {
		match self {
			StatementInput::Match(value) => Some(value.into()),
			StatementInput::Variable(reference) => reference.resolve(root_variable)
		}
	}
}