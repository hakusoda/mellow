use std::fmt::Display;
use serde::{ Serialize, Deserialize };
use futures_util::StreamExt;
use twilight_http::request::{
	channel::reaction::RequestReactionType,
	AuditLogReason
};
use twilight_model::id::Id;

use crate::{
	model::{
		discord::DISCORD_MODELS,
		hakumi::{
			id::{
				marker::DocumentMarker,
				HakuId
			},
			HAKUMI_MODELS
		},
		mellow::MELLOW_MODELS
	},
	server::logging::{ ServerLog, ProfileSyncKind },
	discord::{ CLIENT, INTERACTION },
	syncing::{ MemberStatus, sync_single_user },
	Result
};
use variable::VariableReference;

pub mod stream;
pub mod variable;
pub mod action_tracker;

pub use stream::ElementStream;
pub use variable::{ Variable, VariableKind };
pub use action_tracker::{ ActionTracker, ActionTrackerItem };

#[derive(Clone, Debug, Deserialize)]
pub struct Document {
	pub id: HakuId<DocumentMarker>,
	pub name: String,
	pub kind: DocumentKind,
	pub active: bool,
	pub definition: Vec<Element>
}

impl Document {
	pub async fn process(&self, variables: Variable) -> Result<ActionTracker> {
		let mut stream = ElementStream::new(self.definition.clone(), variables);
		let mut tracker = ActionTracker::new(self.name.clone());
		while let Some((element, variables)) = stream.next().await {
			let result: Result<()> = try {
				match &element.kind {
					ElementKind::BanMember(reference) => {
						if let Some(member) = reference.resolve(&*variables.read().await){
							let user_id = member.get("id").cast_id();
							CLIENT.create_ban(member.get("guild_id").cast_id(), user_id)
								.reason("Triggered by a visual scripting element")?
								.await?;
							tracker.banned_member(user_id);
							break;
						}
					},
					ElementKind::KickMember(reference) => {
						if let Some(member) = reference.resolve(&*variables.read().await) {
							let user_id = member.get("id").cast_id();
							CLIENT.remove_guild_member(member.get("guild_id").cast_id(), user_id)
								.reason("Triggered by a visual scripting element")?
								.await?;
							tracker.kicked_member(user_id);
							break;
						}
					},
					ElementKind::AssignRoleToMember(data) => {
						if let Some(member) = data.reference.resolve(&*variables.read().await) {
							let user_id = member.get("id").cast_id();
							CLIENT.add_guild_member_role(member.get("guild_id").cast_id(), user_id, Id::new(data.value.parse()?))
								.reason("Triggered by a visual scripting element")?
								.await?;
							tracker.assigned_member_role(user_id, &data.value);
						}
					},
					ElementKind::SyncMember => {
						if let Some(member) = Some(variables.read().await.get("member")) {
							let user_id = member.get("id").cast_id();
							let guild_id = member.get("guild_id").cast_id();
							if let Some(user) = HAKUMI_MODELS.user_by_discord(guild_id, user_id).await? {
								let server = MELLOW_MODELS.server(guild_id).await?;
								let member = DISCORD_MODELS.member(guild_id, user_id).await?;
								let result = sync_single_user(server.value(), user.value(), member.value(), None).await?;
								if result.profile_changed || result.member_status.removed() {
									MELLOW_MODELS.server(result.server_id)
										.await?
										.send_logs(vec![ServerLog::ServerProfileSync {
											kind: match result.member_status {
												MemberStatus::Ok => ProfileSyncKind::VisualScripting(self.name.clone()),
												MemberStatus::Banned => ProfileSyncKind::Banned,
												MemberStatus::Kicked => ProfileSyncKind::Kicked
											},
											user_id: member.user_id,
											forced_by: None,
											role_changes: result.role_changes.clone(),
											nickname_change: result.nickname_change.clone(),
											relevant_connections: result.relevant_connections.clone()
										}])
										.await?;
								}
							}
						}
					},
					ElementKind::CreateMessage(data) => {
						let variables = &*variables.read().await;
						if let Some(channel_id) = data.channel_id.resolve(variables) {
							let channel_id = channel_id.cast_id();
							let message = CLIENT.create_message(channel_id)
								.content(&data.content.clone().resolve(variables))?
								.await?
								.model()
								.await?;
							tracker.created_message(channel_id, message.id);
						}
					},
					ElementKind::Reply(data) => {
						if let Some(message) = data.reference.resolve(&*variables.read().await) {
							CLIENT.create_message(message.get("channel_id").cast_id())
								.content(&data.value)?
								.reply(message.get("id").cast_id())
								.await?;
						}
					},
					ElementKind::AddReaction(data) => {
						if let Some(message) = data.reference.resolve(&*variables.read().await) {
							CLIENT.create_reaction(message.get("channel_id").cast_id(), message.get("id").cast_id(), &if data.value.contains(':') {
								let mut split = data.value.split(':');
								RequestReactionType::Custom { name: split.next(), id: Id::new(split.next().unwrap().parse()?) }
							} else { RequestReactionType::Unicode { name: &data.value }}).await?;
						}
					},
					ElementKind::DeleteMessage(data) => {
						if let Some(message) = data.resolve(&*variables.read().await) {
							let channel_id = message.get("channel_id").cast_id();
							CLIENT.delete_message(channel_id, message.get("id").cast_id())
								.reason("Triggered by a visual scripting element")?
								.await?;
							tracker.deleted_message(channel_id, message.get("author").get("id").cast_str());
						}
					},
					ElementKind::GetLinkedPatreonCampaign => {
						let guild_id = variables.read().await.get("guild_id").cast_id();
						let server = MELLOW_MODELS.server(guild_id).await?;
						variables.write().await.set("campaign", crate::patreon::get_campaign(server.oauth_authorisations.first().unwrap()).await?.into());
					},
					ElementKind::InteractionReply(data) => {
						let variables = &*variables.read().await;
						let token = variables.get("interaction_token").cast_str();
						INTERACTION.update_response(token)
							.content(Some(&data.resolve(variables)))?
							.await?;
					},
					_ => ()
				}
			};
			match result {
				Ok(_) => (),
				Err(source) => {
					tracker.error(element.kind, source);
					break;
				}
			}
		}

		Ok(tracker)
	}

	pub fn is_ready_for_stream(&self) -> bool {
		self.active && !self.definition.is_empty()
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

impl ElementKind {
	pub fn display_name(&self) -> &str {
		match self {
			ElementKind::AddReaction(_) => "Add reaction to message",
			ElementKind::AssignRoleToMember(_) => "Assign role to member",
			ElementKind::BanMember(_) => "Ban member from the server",
			ElementKind::Comment => "Comment",
			ElementKind::CreateMessage(_) => "Send message in channel",
			ElementKind::DeleteMessage(_) => "Delete message",
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
	#[serde(rename = "iterable.does_not_contain")]
	DoesNotContain,
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