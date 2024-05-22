use serde::Deserialize;
use chrono::Utc;
use twilight_util::builder::embed::{ ImageSource, EmbedBuilder, EmbedFooterBuilder };
use twilight_model::{
	id::{
		marker::UserMarker,
		Id
	},
	util::Timestamp,
	channel::message::embed::{ Embed, EmbedField, EmbedAuthor, EmbedFooter }
};
use twilight_validate::message::EMBED_COUNT_LIMIT;

use super::action_log::ActionLog;
use crate::{
	model::{
		discord::{
			user::CachedUser,
			DISCORD_MODELS
		},
		hakumi::{
			user::Connection,
			HAKUMI_MODELS
		},
		mellow::{
			server::Server,
			MELLOW_MODELS
		}
	},
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
		user_id: Id<UserMarker>,
		forced_by: Option<Id<UserMarker>>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<Connection>
	} = 1 << 1,
	#[serde(skip)]
	UserCompletedOnboarding {
		user_id: Id<UserMarker>
	} = 1 << 2,
	#[serde(skip)]
	VisualScriptingDocumentResult {
		items: Vec<ActionTrackerItem>,
		document_name: String
	} = 1 << 3
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
				if value == 4 || (self.logging_types & value) == value {
					match log {
						ServerLog::ActionLog(payload) => {
							if let Some(document) = payload.target_document.clone() {
								let id = document.id;
								let kind = document.kind.clone();
								HAKUMI_MODELS.vs_documents.insert(id, document);
								MELLOW_MODELS.event_documents.insert((self.id, kind), Some(id));
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
						ServerLog::ServerProfileSync { kind, user_id, forced_by, role_changes, nickname_change, relevant_connections } => {
							let user = DISCORD_MODELS.user(user_id).await?;
							let mut embed = EmbedBuilder::new()
								.title(match kind {
									ProfileSyncKind::Default => if let Some(forced_by) = forced_by && forced_by != user_id {
										let other_user = DISCORD_MODELS.user(forced_by).await?;
										format!("{} forcefully synced {}'s profile", other_user.display_name(), user.display_name())
									} else { format!("{} synced their profile", user.display_name()) },
									ProfileSyncKind::VisualScripting(name) => format!("{} was synced by <:document:1222904218499940395> {name}", user.display_name()),
									ProfileSyncKind::Banned => format!("{} has been banned", user.display_name()),
									ProfileSyncKind::Kicked => format!("{} has been kicked", user.display_name())
								})
								.footer(self.embed_footer(user.value(), Some("Member Sync Result")))
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
						ServerLog::UserCompletedOnboarding { user_id } => {
							let user = DISCORD_MODELS.user(user_id).await?;
							embeds.push(EmbedBuilder::new()
								.title(format!("{} completed onboarding", user.display_name()))
								.author(self.embed_author(user.value(), None))
								.build()
							);
						},
						ServerLog::VisualScriptingDocumentResult { items, document_name } => {
							embeds.push(EmbedBuilder::new()
								.title(format!("Result for <:document:1222904218499940395> {document_name}"))
								.footer(EmbedFooter {
									text: "Visual Scripting Output".into(),
									icon_url: None,
									proxy_icon_url: None
								})
								.description(items
									.into_iter()
									.map(|x| match x {
										ActionTrackerItem::Error(element_kind, source) =>
											format!("* Encountered an error at **{}**\n```diff\n- {}\n--- {}```\n", element_kind.display_name(), source.kind, source.context),
										ActionTrackerItem::AssignedMemberRole(user_id, role_id) =>
											format!("* Assigned <@&{role_id}> to <@{user_id}>"),
										ActionTrackerItem::RemovedMemberRole(user_id, role_id) =>
											format!("* Removed <@&{role_id}> from <@{user_id}>"),
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

	fn embed_author(&self, user: &CachedUser, title: Option<String>) -> EmbedAuthor {
		EmbedAuthor {
			url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", self.id, user.id)),
			name: title.unwrap_or_else(|| user.display_name().into()),
			icon_url: user.avatar_url(),
			proxy_icon_url: None
		}
	}

	fn embed_footer(&self, user: &CachedUser, title: Option<&str>) -> EmbedFooter {
		EmbedFooter {
			text: title.map(|x| x.to_string()).unwrap_or_else(|| user.display_name().into()),
			icon_url: user.avatar_url(),
			proxy_icon_url: None
		}
	}
}

pub enum ProfileSyncKind {
	Default,
	VisualScripting(String),
	Banned,
	Kicked
}

impl Default for ProfileSyncKind {
	fn default() -> Self {
		Self::Default
	}
}