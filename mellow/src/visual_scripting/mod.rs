use serde::{ Serialize, Deserialize };

use variable::VariableReference;

pub mod stream;
pub mod variable;
pub mod action_tracker;

pub use stream::ElementStream;
pub use variable::{ Variable, VariableKind };
pub use action_tracker::{ ActionTracker, ActionTrackerItem };

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
	#[serde(rename = "mellow.command")]
	MellowCommand,

	#[serde(rename = "mellow.discord_event.member_join")]
	MemberJoinEvent,
	#[serde(rename = "mellow.discord_event.message_create")]
	MessageCreatedEvent,
	#[serde(rename = "mellow.discord_event.member.completed_onboarding")]
	MemberCompletedOnboardingEvent,

	#[serde(rename = "mellow.event.member.synced")]
	MemberSynced
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

	#[serde(rename = "action.mellow.message.create")]
	CreateMessage(Message),
	#[serde(rename = "action.mellow.message.delete")]
	DeleteMessage(VariableReference),

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


#[derive(Clone, Debug, Deserialize)]
pub struct StringValueWithVariableReference {
	pub value: String,
	pub reference: VariableReference
}

#[derive(Clone, Debug, Deserialize)]
pub struct Message {
	pub content: Text,
	pub channel_id: StatementInput
}

#[derive(Clone, Debug, Deserialize)]
pub struct Text {
	pub value: Vec<TextElement>
}

impl Text {
	pub fn resolve(self, root_variable: &Variable) -> String {
		self.value.into_iter().map(|x| match x {
			TextElement::String(x) => x,
			TextElement::Variable(x) => x.resolve(root_variable).unwrap().cast_string()
		}).collect::<Vec<String>>().join("")
	}
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum TextElement {
	String(String),
	Variable(VariableReference)
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
	pub fn resolve(&self, root_variable: &Variable) -> Option<Variable> {
		match self {
			StatementInput::Match(value) => Some(value.into()),
			StatementInput::Variable(reference) => reference.resolve(&root_variable)
		}
	}
}