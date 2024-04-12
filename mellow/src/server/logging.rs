use serde::{ Serialize, Deserialize };
use chrono::Utc;
use tracing::{ Instrument, info_span };
use twilight_util::builder::embed::{ ImageSource, EmbedBuilder, EmbedFooterBuilder };
use twilight_model::{
	id::{
		marker::UserMarker,
		Id
	},
	util::Timestamp,
	guild::PartialMember,
	channel::message::embed::{ Embed, EmbedField, EmbedAuthor, EmbedFooter }
};
use twilight_validate::message::EMBED_COUNT_LIMIT;

use super::action_log::ActionLog;
use crate::{
	util::WithId,
	cache::CACHES,
	model::{
		hakumi::user::Connection,
		mellow::server::Server
	},
	traits::{ QuickId, AvatarUrl, DisplayName },
	syncing::{ RoleChange, RoleChangeKind, NicknameChange },
	discord::CLIENT,
	visual_scripting::ActionTrackerItem,
	Result
};

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
#[repr(u8)]
pub enum ServerLog {
	ActionLog(ActionLog) = 1 << 0,
	#[serde(skip)]
	ServerProfileSync {
		kind: ProfileSyncKind,
		member: WithId<Id<UserMarker>, PartialMember>,
		forced_by: Option<PartialMember>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<Connection>
	} = 1 << 1,
	UserCompletedOnboarding {
		member: PartialMember
	} = 1 << 2,
	EventResponseResult {
		invoker: PartialMember,
		event_kind: String,
		member_result: EventResponseResultMemberResult
	} = 1 << 3,
	VisualScriptingProcessorError {
		error: String,
		document_name: String
	} = 1 << 4,
	#[serde(skip)]
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
		
		if let Some(channel_id) = self.logging_channel_id {
			let mut embeds: Vec<Embed> = vec![];
			for log in logs {
				let value = log.discriminant();
				if value == 4 || value == 8 || value == 16 || value == 32 || (self.logging_types & value) == value {
					match log {
						ServerLog::ActionLog(payload) => {
							if let Some(document) = payload.target_document.clone() {
								let cache_key = (self.id, document.kind.clone());
								let span = info_span!("cache.event_responses.write", ?cache_key);
								CACHES.event_responses.insert(cache_key, document)
									.instrument(span)
									.await;
							}

							let mut footer = EmbedFooterBuilder::new("Action Log");
							if let Some(url) = payload.author.as_ref().and_then(|x| x.avatar_url.as_ref()) {
								footer = footer.icon_url(ImageSource::url(url)?);
							}

							embeds.push(EmbedBuilder::new()
								.footer(footer)
								.timestamp(Timestamp::from_secs(Utc::now().timestamp())?)
								.description(format!("### {} {}\n{}",
									if let Some(ref author) = payload.author {
										format!("[{}](https://hakumi.cafe/user/{})",
											author.display_name(),
											author.username
										)
									} else { "<:hakumi_squircled:1226111994655150090>  HAKUMI".into() },
									payload.action_string(&self.id),
									payload.details().join("\n")
								))
								.build()
							);
						},
						ServerLog::ServerProfileSync { kind, member, forced_by, role_changes, nickname_change, relevant_connections } => {
							let mut embed = EmbedBuilder::new()
								.title(match kind {
									ProfileSyncKind::Default => forced_by.and_then(|x| if x.id() == member.id() { None } else { Some(x) }).map_or_else(
										|| format!("{} synced their profile", member.display_name()),
										|x| format!("{} forcefully synced {}'s profile", x.display_name(), member.display_name())
									),
									ProfileSyncKind::NewMember => format!("{} joined and has been synced", member.display_name())
								})
								.footer(self.embed_footer(&member, Some("Member Sync Result")))
								.timestamp(Timestamp::from_secs(Utc::now().timestamp())?);
							if !role_changes.is_empty() {
								embed = embed.field(EmbedField {
									name: "Role changes".into(),
									value: format!("```diff\n{}```", role_changes.iter().map(|x| match x.kind {
										RoleChangeKind::Added => format!("+ {}", x.display_name),
										RoleChangeKind::Removed => format!("- {}", x.display_name)
									}).collect::<Vec<String>>().join("\n")),
									inline: false
								});
							}
							if let Some(changes) = nickname_change {
								embed = embed.field(EmbedField {
									name: "Nickname changes".into(),
									value: format!("```diff{}{}```",
										changes.0.map(|x| format!("\n- {x}")).unwrap_or("".into()),
										changes.1.map(|x| format!("\n+ {x}")).unwrap_or("".into())
									),
									inline: false
								});
							}
							if !relevant_connections.is_empty() {
								embed = embed.field(EmbedField {
									name: "Relevant connections".into(),
									value: relevant_connections.iter().map(|x| x.display()).collect::<Vec<String>>().join("\n"),
									inline: false
								});
							}
	
							embeds.push(embed.build());
						},
						ServerLog::UserCompletedOnboarding { member } => {
							embeds.push(EmbedBuilder::new()
								.title(format!("{} completed onboarding", member.display_name()))
								.author(self.embed_author(&member, None))
								.build()
							);
						},
						ServerLog::EventResponseResult { invoker, event_kind, member_result} => {
							embeds.push(EmbedBuilder::new()
								.title(match member_result {
									EventResponseResultMemberResult::Banned => format!("{} was banned", invoker.display_name()),
									EventResponseResultMemberResult::Kicked => format!("{} was kicked", invoker.display_name()),
									_ => "no result".into()
								})
								.author(self.embed_author(&invoker, Some(format!("Event Response Result ({event_kind})"))))
								.build()
							);
						},
						ServerLog::VisualScriptingProcessorError { error, document_name } => {
							embeds.push(EmbedBuilder::new()
								.title(format!("The Visual Scripting Document named “{document_name}” encountered an error while being processed, tragic..."))
								.description(error)
								.build()
							);
						},
						ServerLog::VisualScriptingDocumentResult { items, document_name } => {
							embeds.push(EmbedBuilder::new()
								.title(format!("Result for <:document:1222904218499940395> {document_name}"))
								.footer(EmbedFooter {
									text: "Visual Script Processor Result".into(),
									icon_url: None,
									proxy_icon_url: None
								})
								.description(items
									.into_iter()
									.map(|x| match x {
										ActionTrackerItem::Error(source) =>
											format!("* Encountered an error (uh oh!)\n```diff\n- {}\n--- {}```\n", source.kind, source.context),
										ActionTrackerItem::AssignedMemberRole(user_id, role_id) =>
											format!("* Assigned <@&{role_id}> to <@{user_id}>"),
										ActionTrackerItem::BannedMember(user_id) =>
											format!("* Banned <@{user_id}> from the server"),
										ActionTrackerItem::KickedMember(user_id) =>
											format!("* Kicked <@{user_id}> from the server"),
										ActionTrackerItem::CreatedMessage(channel_id, message_id) =>
											format!("* Sent a message in <#{channel_id}>: https://discord.com/channels/{}/{channel_id}/{message_id}", self.id),
										ActionTrackerItem::DeletedMessage(channel_id, user_id) =>
											format!("* Deleted a message in <#{channel_id}> by <@{user_id}>")
									})
									.collect::<Vec<String>>()
									.join("\n")
								)
								.build()
							);
						}
					}
				}
			}
	
			if !embeds.is_empty() {
				for chunk in embeds.chunks(EMBED_COUNT_LIMIT) {
					CLIENT.create_message(channel_id)
						.embeds(chunk)?
						.await?;
				}
			}
		}

		Ok(())
	}

	fn embed_author(&self, member: &PartialMember, title: Option<String>) -> EmbedAuthor {
		EmbedAuthor {
			url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", self.id, member.id())),
			name: title.unwrap_or_else(|| member.display_name().into()),
			icon_url: member.avatar_url(),
			proxy_icon_url: None
		}
	}

	fn embed_footer(&self, member: &PartialMember, title: Option<&str>) -> EmbedFooter {
		EmbedFooter {
			text: title.map(|x| x.to_string()).unwrap_or_else(|| member.display_name().into()),
			icon_url: member.avatar_url(),
			proxy_icon_url: None
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