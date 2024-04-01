use serde::{ Serialize, Deserialize };
use tracing::{ Instrument, info_span};

use super::{ action_log::ActionLog, Server };
use crate::{
	util::unwrap_string_or_array,
	cache::CACHES,
	syncing::{ RoleChange, RoleChangeKind, NicknameChange },
	discord::{ DiscordMember, ChannelMessage, create_channel_message },
	database::UserConnection,
	interaction::{ Embed, EmbedField, EmbedAuthor },
	visual_scripting::ActionTrackerItem,
	Result
};

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", content = "data")]
#[repr(u8)]
pub enum ServerLog {
	ActionLog(ActionLog) = 1 << 0,
	ServerProfileSync {
		#[serde(skip)]
		kind: ProfileSyncKind,
		member: DiscordMember,
		forced_by: Option<DiscordMember>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<UserConnection>
	} = 1 << 1,
	UserCompletedOnboarding {
		member: DiscordMember
	} = 1 << 2,
	EventResponseResult {
		invoker: DiscordMember,
		event_kind: String,
		member_result: EventResponseResultMemberResult
	} = 1 << 3,
	VisualScriptingProcessorError {
		error: String,
		document_name: String
	} = 1 << 4,
	VisualScriptingDocumentResult {
		items: Vec<ActionTrackerItem>,
		document_name: String
	} = 1 << 5
}

#[derive(Deserialize, Serialize)]
pub enum EventResponseResultMemberResult {
	None,
	Banned,
	Kicked
}

impl ServerLog {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
}

impl Server {
	pub async fn send_logs(&self, logs: Vec<ServerLog>) -> Result<()> {
		if logs.is_empty() {
			return Ok(());
		}
		
		if let Some(channel_id) = &self.logging_channel_id {
			let mut embeds: Vec<Embed> = vec![];
			for log in logs {
				let value = log.discriminant();
				if value == 4 || value == 8 || value == 16 || value == 32 || (self.logging_types & value) == value {
					match log {
						ServerLog::ActionLog(payload) => {
							let website_url = format!("https://hakumi.cafe/mellow/server/{}/settings/action_log", self.id);
							if let Some(document) = payload.target_document.clone() {
								let cache_key = (self.id.clone(), document.kind.clone());
								let span = info_span!("cache.event_responses.write", ?cache_key);
								CACHES.event_responses.insert(cache_key, document)
									.instrument(span)
									.await;
							}

							let mut details: Vec<String> = vec![];
							if payload.kind == "mellow.server.syncing.action.created" {
								if let Some(name) = payload.data.get("name").and_then(unwrap_string_or_array) {
									details.push(format!("* With name **{name}**"));
								}
								if let Some(requirements) = payload.data.get("requirements").and_then(|x| x.as_i64()) {
									details.push(format!("* With {requirements} requirement(s)"));
								}
							}
							if payload.kind == "mellow.server.syncing.action.created" || payload.kind == "mellow.server.syncing.action.updated" || payload.kind == "mellow.server.syncing.settings.updated" || payload.kind == "mellow.server.discord_logging.updated" || payload.kind == "mellow.server.automation.event.updated" {
								details.push(format!("* *View the full details [here]({website_url})*"));
							}

							embeds.push(Embed {
								author: Some(EmbedAuthor {
									url: Some(website_url),
									name: Some("New Action Log".into()),
									icon_url: payload.author.avatar_url.clone()
								}),
								description: Some(format!("### [{}](https://hakumi.cafe/user/{}) {}\n{}",
									payload.author.display_name(),
									payload.author.username,
									payload.action_string(&self.id),
									details.join("\n")
								)),
								..Default::default()
							});
						},
						ServerLog::ServerProfileSync { kind, member, forced_by, role_changes, nickname_change, relevant_connections } => {
							let mut fields: Vec<EmbedField> = vec![];
							if !role_changes.is_empty() {
								fields.push(EmbedField {
									name: "Role changes".into(),
									value: format!("```diff\n{}```", role_changes.iter().map(|x| match x.kind {
										RoleChangeKind::Added => format!("+ {}", x.display_name),
										RoleChangeKind::Removed => format!("- {}", x.display_name)
									}).collect::<Vec<String>>().join("\n")),
									inline: None
								});
							}
							if let Some(changes) = nickname_change {
								fields.push(EmbedField {
									name: "Nickname changes".into(),
									value: format!("```diff{}{}```",
										changes.0.map(|x| format!("\n- {x}")).unwrap_or("".into()),
										changes.1.map(|x| format!("\n+ {x}")).unwrap_or("".into())
									),
									inline: None
								});
							}
							if !relevant_connections.is_empty() {
								fields.push(EmbedField {
									name: "Relevant connections".into(),
									value: relevant_connections.iter().map(|x| x.display()).collect::<Vec<String>>().join("\n"),
									inline: None
								});
							}
	
							embeds.push(Embed {
								title: Some(match kind {
									ProfileSyncKind::Default => forced_by.and_then(|x| if x.id() == member.id() { None } else { Some(x) }).map_or_else(
										|| format!("{} synced their profile", member.display_name()),
										|x| format!("{} forcefully synced {}'s profile", x.display_name(), member.display_name())
									),
									ProfileSyncKind::NewMember => format!("{} joined and has been synced", member.display_name())
								}),
								author: Some(self.embed_author(&member, None)),
								fields: Some(fields),
								..Default::default()
							});
						},
						ServerLog::UserCompletedOnboarding { member } => {
							embeds.push(Embed {
								title: Some(format!("{} completed onboarding", member.display_name())),
								author: Some(self.embed_author(&member, None)),
								..Default::default()
							});
						},
						ServerLog::EventResponseResult { invoker, event_kind, member_result} => {
							embeds.push(Embed {
								title: Some(match member_result {
									EventResponseResultMemberResult::Banned => format!("{} was banned", invoker.display_name()),
									EventResponseResultMemberResult::Kicked => format!("{} was kicked", invoker.display_name()),
									_ => "no result".into()
								}),
								author: Some(self.embed_author(&invoker, Some(format!("Event Response Result ({event_kind})")))),
								..Default::default()
							});
						},
						ServerLog::VisualScriptingProcessorError { error, document_name } => {
							embeds.push(Embed {
								title: Some(format!("The Visual Scripting Document named “{document_name}” encountered an error while being processed, tragic...")),
								description: Some(error),
								..Default::default()
							});
						},
						ServerLog::VisualScriptingDocumentResult { items, document_name } => {
							embeds.push(Embed {
								title: Some(format!("Result for <:document:1222904218499940395> {document_name}")),
								description: Some(items
									.into_iter()
									.map(|x| match x {
										ActionTrackerItem::AssignedMemberRole(user_id, role_id) =>
											format!("* Assigned <@&{role_id}> to <@{user_id}>"),
										ActionTrackerItem::BannedMember(user_id) =>
											format!("* Banned <@{user_id}> from the server"),
										ActionTrackerItem::KickedMember(user_id) =>
											format!("* Kicked <@{user_id}> from the server")
									})
									.collect::<Vec<String>>()
									.join("\n")
								),
								..Default::default()
							});
						}
					}
				}
			}
	
			if !embeds.is_empty() {
				for chunk in embeds.chunks(10) {
					create_channel_message(channel_id, ChannelMessage {
						embeds: Some(chunk.to_vec()),
						..Default::default()
					}).await?;
				}
			}
		}

		Ok(())
	}

	fn embed_author(&self, member: &DiscordMember, title: Option<String>) -> EmbedAuthor {
		EmbedAuthor {
			url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", self.id, member.id())),
			name: title.or(member.user.global_name.clone()),
			icon_url: member.avatar.as_ref().or(member.user.avatar.as_ref()).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp?size=48", member.id())),
			..Default::default()
		}
	}
}

pub enum ProfileSyncKind {
	Default,
	NewMember
}

impl Default for ProfileSyncKind {
	fn default() -> Self {
		Self::Default
	}
}