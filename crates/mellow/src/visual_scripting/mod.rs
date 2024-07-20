use futures::StreamExt;
use mellow_cache::CACHE;
use mellow_models::hakumi::visual_scripting::{ variable::VariableInterpretAs, DocumentModel, ElementKind, Variable, VariableKind };
use mellow_util::{ DISCORD_CLIENT, DISCORD_INTERACTION_CLIENT };
use twilight_http::request::{
	channel::reaction::RequestReactionType,
	AuditLogReason
};
use twilight_model::id::{
	marker::{ GuildMarker, UserMarker },
	Id
};

use crate::{
	server::logging::send_logs,
	syncing::{ SyncingInitiator, sync_single_user },
	Error, Result
};

pub mod action_tracker;
pub use action_tracker::{ ActionTracker, ActionTrackerItem };

mod stream;
use stream::ElementStream;

pub async fn process_document(document: DocumentModel, variables: Variable) -> ActionTracker {
	let mut stream = ElementStream::new(document.definition.clone(), variables);
	let mut tracker = ActionTracker::new(document.name.clone());
	while let Some((element, variables)) = stream.next().await {
		let result: Result<()> = try {
			match &element.kind {
				ElementKind::BanMember(reference) => {
					if let Some(member) = reference.resolve(&*variables.read().await){
						let user_id = member.get("id").cast_id();
						DISCORD_CLIENT
							.create_ban(member.get("guild_id").cast_id(), user_id)
							.reason("Triggered by a visual scripting element")
							.await?;
						tracker.banned_member(user_id);
						break;
					}
				},
				ElementKind::KickMember(reference) => {
					if let Some(member) = reference.resolve(&*variables.read().await) {
						let user_id = member.get("id").cast_id();
						DISCORD_CLIENT
							.remove_guild_member(member.get("guild_id").cast_id(), user_id)
							.reason("Triggered by a visual scripting element")
							.await?;
						tracker.kicked_member(user_id);
						break;
					}
				},
				ElementKind::AssignRoleToMember(data) => {
					if let Some(member) = data.reference.resolve(&*variables.read().await) {
						let user_id = member.get("id").cast_id();
						DISCORD_CLIENT
							.add_guild_member_role(member.get("guild_id").cast_id(), user_id, Id::new(data.value.parse()?))
							.reason("Triggered by a visual scripting element")
							.await?;
						tracker.assigned_member_role(user_id, &data.value);
					}
				},
				ElementKind::RemoveRoleFromMember(data) => {
					if let Some(member) = data.reference.resolve(&*variables.read().await) {
						let user_id = member.get("id").cast_id();
						DISCORD_CLIENT
							.remove_guild_member_role(member.get("guild_id").cast_id(), user_id, Id::new(data.value.parse()?))
							.reason("Triggered by a visual scripting element")
							.await?;
						tracker.removed_member_role(user_id, &data.value);
					}
				},
				ElementKind::SyncMember => {
					if let Some(member) = Some(variables.read().await.get("member")) {
						let user_id = member.get("id").cast_id();
						let guild_id = member.get("guild_id").cast_id();
						if let Some(haku_id) = CACHE.hakumi.user_by_discord(guild_id, user_id) .await? {
							let result = sync_single_user(guild_id, haku_id, user_id, SyncingInitiator::VisualScriptingDocument(document.id), None)
								.await?;
							if let Some(result_log) = result.create_log() {
								send_logs(guild_id, vec![result_log])
									.await?;
							}
						}
					}
				},
				ElementKind::CreateMessage(data) => {
					let variables = &*variables.read().await;
					if let Some(channel_id) = data.channel_id.resolve(variables) {
						let channel_id = channel_id.cast_id();
						let message = DISCORD_CLIENT.create_message(channel_id)
							.content(&data.content.clone().resolve(variables))
							.await?
							.model()
							.await?;
						tracker.created_message(channel_id, message.id);
					}
				},
				ElementKind::Reply(data) => {
					if let Some(message) = data.reference.resolve(&*variables.read().await) {
						DISCORD_CLIENT
							.create_message(message.get("channel_id").cast_id())
							.content(&data.value)
							.reply(message.get("id").cast_id())
							.await?;
					}
				},
				ElementKind::AddReaction(data) => {
					if let Some(message) = data.reference.resolve(&*variables.read().await) {
						let emoji = if data.value.contains(':') {
							let mut split = data.value.split(':');
							RequestReactionType::Custom { name: split.next(), id: Id::new(split.next().unwrap().parse()?) }
						} else {
							RequestReactionType::Unicode { name: &data.value }
						};
						DISCORD_CLIENT
							.create_reaction(message.get("channel_id").cast_id(), message.get("id").cast_id(), &emoji)
							.await?;
					}
				},
				ElementKind::DeleteMessage(data) => {
					if let Some(message) = data.resolve(&*variables.read().await) {
						let channel_id = message.get("channel_id").cast_id();
						DISCORD_CLIENT
							.delete_message(channel_id, message.get("id").cast_id())
							.reason("Triggered by a visual scripting element")
							.await?;
						tracker.deleted_message(channel_id, message.get("author").get("id").cast_str());
					}
				},
				ElementKind::GetLinkedPatreonCampaign => {
					let guild_id = variables.read().await.get("guild_id").cast_id();
					let campaign = CACHE
						.patreon
						.campaign(guild_id)
						.await?
						.ok_or(Error::PatreonCampaignNotConnected)?
						.clone();
					variables
						.write()
						.await
						.set("campaign", campaign.into());
				},
				ElementKind::InteractionReply(data) => {
					let variables = &*variables.read().await;
					let token = variables.get("interaction_token").cast_str();
					DISCORD_INTERACTION_CLIENT
						.update_response(token)
						.content(Some(&data.resolve(variables)))
						.await?;

					tracker.replied = true;
				},
				ElementKind::StartThreadFromMessage { name, message } => {
					let variables = &*variables.read().await;
					if let Some(message) = message.resolve(variables) {
						let channel_id = message.get("channel_id").cast_id();
						let new_thread = DISCORD_CLIENT
							.create_thread_from_message(channel_id, message.get("id").cast_id(), &name.resolve(variables))
							.await?
							.model()
							.await?;
						tracker.created_thread(channel_id, new_thread.id);
					}
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

	tracker
}

pub async fn variable_from_member(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Variable> {
	let user = CACHE
		.discord
		.user(user_id)
		.await?;
	let user_name = user.name.clone();
	let user_global_name = user.global_name.clone();

	let member = CACHE
		.discord
		.member(guild_id, user_id)
		.await?;
	Ok(Variable::create_map([
		("id", user_id.to_string().into()),
		("roles", VariableKind::List(member.roles.iter().map(|x| x.to_string().into()).collect()).into()),
		("guild_id", guild_id.to_string().into()),
		("username", user_name.clone().into()),
		("avatar_url", member.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", user_id)).unwrap_or("".into()).into()),
		("display_name", user_global_name.unwrap_or(user_name).into())
	], Some(VariableInterpretAs::Member)))
}