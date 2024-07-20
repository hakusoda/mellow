use chrono::Utc;
use mellow_cache::CACHE;
use mellow_models::discord::UserModel;
use mellow_util::{
	hakuid::{
		marker::ConnectionMarker,
		HakuId
	},
	DISCORD_CLIENT
};
use serde::Deserialize;
use twilight_model::{
	id::{
		marker::{ GuildMarker, UserMarker },
		Id
	},
	util::Timestamp,
	channel::message::embed::{ Embed, EmbedField, EmbedAuthor, EmbedFooter }
};
use twilight_util::builder::embed::{ ImageSource, EmbedBuilder, EmbedFooterBuilder };
use twilight_validate::message::EMBED_COUNT_LIMIT;

use super::action_log::ActionLog;
use crate::{
	syncing::{ NicknameChange, RoleChange, RoleChangeKind, SyncingInitiator },
	visual_scripting::ActionTrackerItem,
	Error, Result
};

#[derive(Deserialize)]
#[serde(tag = "type", content = "data")]
#[repr(u8)]
pub enum ServerLog {
	ActionLog(ActionLog) = 1 << 0,
	#[serde(skip)]
	ServerProfileSync {
		kind: ProfileSyncKind,
		initiator: SyncingInitiator,
		user_id: Id<UserMarker>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<HakuId<ConnectionMarker>>
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

pub async fn send_logs(guild_id: Id<GuildMarker>, logs: Vec<ServerLog>) -> Result<()> {
	if logs.is_empty() {
		return Ok(());
	}
	
	let server = CACHE
		.mellow
		.server(guild_id)
		.ok_or(Error::ServerNotFound)?;
	if let Some(channel_id) = server.logging_channel_id {
		let logging_types = server.logging_types;

		let mut embeds: Vec<Embed> = vec![];
		for log in logs {
			let value = log.discriminant();
			if value == 4 || (logging_types & value) == value {
				match log {
					ServerLog::ActionLog(payload) => {
						if let Some(document) = payload.target_document.clone() {
							let id = document.id;
							//let kind = document.kind.clone();
							CACHE.hakumi.visual_scripting_documents.insert(id, document);
							//MELLOW_MODELS.event_documents.insert((guild_id, kind), Some(id));
						}

						let mut footer = EmbedFooterBuilder::new("Action Log");
						if
							let Some(user_id) = payload.author &&
							let Some(url) = CACHE
								.hakumi
								.user(user_id)
								.await?
								.avatar_url
								.clone()
						{
							footer = footer.icon_url(ImageSource::url(url)?);
						}

						embeds.push(EmbedBuilder::new()
							.footer(footer)
							.timestamp(Timestamp::from_secs(Utc::now().timestamp())?)
							.description(format!("### {} {}\n{}",
								if let Some(user_id) = payload.author {
									let author = CACHE
										.hakumi
										.user(user_id)
										.await?;
									format!("[{}](https://hakumi.cafe/user/{})",
										author.display_name(),
										author.username
									)
								} else { "<:hakumi_squircled:1226111994655150090>  HAKUMI".into() },
								payload.action_string(guild_id),
								payload.details().join("\n")
							))
							.build()
						);
					},
					ServerLog::ServerProfileSync { kind, initiator, user_id, role_changes, nickname_change, relevant_connections } => {
						let title = match kind {
							ProfileSyncKind::Default => match initiator {
								SyncingInitiator::Automatic =>
									format!("<@{user_id}> was automatically synced"),
								SyncingInitiator::ForcedBy(other_user_id) =>
									format!("<@{other_user_id}> forcefully synced <@{user_id}>'s profile"),
								SyncingInitiator::Manual =>
									format!("<@{user_id}> synced their profile"),
								SyncingInitiator::VisualScriptingDocument(document_id) => {
									let document = CACHE
										.hakumi
										.visual_scripting_document(document_id)
										.await?;
									format!("<@{user_id}> was synced by <:document:1222904218499940395> {}", document.name)
								}
							},
							ProfileSyncKind::Banned =>
								format!("<@{user_id}> has been banned"),
							ProfileSyncKind::Kicked =>
								format!("<@{user_id}> has been kicked")
						};
						let mut embed = EmbedBuilder::new()
							.description(format!("### {title}"))
							.footer(embed_footer(user_id, Some("Member Sync Result")).await?)
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
							let connections = CACHE
								.hakumi
								.connections(&relevant_connections)
								.await?;
							embed = embed.field(EmbedField {
								name: "Relevant connections".into(),
								value: connections.into_iter().map(|x| x.display()).collect::<Vec<String>>().join("\n"),
								inline: false
							});
						}

						embeds.push(embed.build());
					},
					ServerLog::UserCompletedOnboarding { user_id } => {
						let user = CACHE
							.discord
							.user(user_id)
							.await?;
						embeds.push(EmbedBuilder::new()
							.title(format!("{} completed onboarding", user.display_name()))
							.author(embed_author(guild_id, user.value(), None))
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
										format!("* Encountered an error at **{}**\n```diff\n- {}```\n", element_kind.display_name(), source),
									ActionTrackerItem::AssignedMemberRole(user_id, role_id) =>
										format!("* Assigned <@&{role_id}> to <@{user_id}>"),
									ActionTrackerItem::RemovedMemberRole(user_id, role_id) =>
										format!("* Removed <@&{role_id}> from <@{user_id}>"),
									ActionTrackerItem::BannedMember(user_id) =>
										format!("* Banned <@{user_id}> from the server"),
									ActionTrackerItem::KickedMember(user_id) =>
										format!("* Kicked <@{user_id}> from the server"),
									ActionTrackerItem::CreatedMessage(channel_id, message_id) =>
										format!("* Sent a message in <#{channel_id}>: https://discord.com/channels/{guild_id}/{channel_id}/{message_id}"),
									ActionTrackerItem::DeletedMessage(channel_id, user_id) =>
										format!("* Deleted a message in <#{channel_id}> by <@{user_id}>"),
									ActionTrackerItem::CreatedThread(channel_id, thread_id) =>
										format!("* Started a new thread in <#{channel_id}>: <#{thread_id}>")
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
				DISCORD_CLIENT.create_message(channel_id)
					.embeds(chunk)
					.await?;
			}
		}
	}

	Ok(())
}

fn embed_author(guild_id: Id<GuildMarker>, user: &UserModel, title: Option<String>) -> EmbedAuthor {
	EmbedAuthor {
		url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", guild_id, user.id)),
		name: title.unwrap_or_else(|| user.display_name().into()),
		icon_url: user.avatar_url(),
		proxy_icon_url: None
	}
}

async fn embed_footer(user_id: Id<UserMarker>, title: Option<&str>) -> Result<EmbedFooter> {
	let user = CACHE
		.discord
		.user(user_id)
		.await?;
	Ok(EmbedFooter {
		text: title.map(|x| x.to_string()).unwrap_or_else(|| user.display_name().into()),
		icon_url: user.avatar_url(),
		proxy_icon_url: None
	})
}

pub enum ProfileSyncKind {
	Default,
	Banned,
	Kicked
}

impl Default for ProfileSyncKind {
	fn default() -> Self {
		Self::Default
	}
}