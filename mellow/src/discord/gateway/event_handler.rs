use std::time::SystemTime;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	gateway::payload::incoming::{ MemberAdd, MemberUpdate, MessageCreate }
};

use crate::{
	server::logging::{ ServerLog, ProfileSyncKind },
	syncing::sync_single_user,
	discord::{
		Guild, GuildMember, ChannelMessage, GuildOnboarding, MessageReference, GuildVerificationLevel,
		ban_member, remove_member, delete_message, assign_member_role, create_channel_message, create_message_reaction
	},
	database,
	visual_scripting::{ Element, Variable, ElementKind, DocumentKind, ActionTracker },
	Result,
	PENDING_VERIFICATION_TIMER
};

pub async fn process_element_for_member(element: &Element, variables: &Variable, tracker: &mut ActionTracker) -> Result<bool> {
	Ok(match &element.kind {
		ElementKind::BanMember(reference) => {
			if let Some(member) = reference.resolve(&variables){
				let user_id = member.get("id").cast_str();
				ban_member(member.get("guild_id").cast_str(), user_id).await?;
				tracker.banned_member(user_id);
				true
			} else { false }
		},
		ElementKind::KickMember(reference) => {
			if let Some(member) = reference.resolve(&variables) {
				let user_id = member.get("id").cast_str();
				remove_member(member.get("guild_id").cast_str(), user_id).await?;
				tracker.kicked_member(user_id);
				true
			} else { false }
		},
		ElementKind::AssignRoleToMember(data) => {
			if let Some(member) = data.reference.resolve(&variables) {
				let user_id = member.get("id").cast_str();
				assign_member_role(member.get("guild_id").cast_str(), user_id, &data.value).await?;
				tracker.assigned_member_role(user_id, &data.value);
				true
			} else { false }
		},
		_ => false
	})
}

static PENDING_MEMBERS: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>)>> = RwLock::const_new(vec![]);

pub async fn member_add(event_data: &MemberAdd) -> Result<()> {
	let user_id = &event_data.user.id;
	let guild_id = &event_data.guild_id;
	if event_data.member.pending {
		PENDING_MEMBERS.write().await.push((guild_id.clone(), user_id.clone()));
	}

	let document = database::get_server_event_response_tree(guild_id, DocumentKind::MemberJoinEvent).await?;
	if document.is_ready_for_stream() {
		if let Some(user) = database::get_user_by_discord(guild_id, user_id).await? {
			// TODO: this member get is pointless, replace with event_data.member
			let member = GuildMember::fetch(guild_id, user_id).await?;
			let (mut stream, mut tracker) = document.into_stream(Variable::create_map([
				("member", member.into_variable(guild_id))
			], None));
			while let Some((element, variables)) = stream.next().await {
				if process_element_for_member(&element, &variables, &mut tracker).await? { break }
				match element.kind {
					ElementKind::SyncMember => {
						let result = sync_single_user(&user, &member, guild_id, None).await?;
						if result.profile_changed {
							result.server.send_logs(vec![ServerLog::ServerProfileSync {
								kind: ProfileSyncKind::NewMember,
								member: member.clone(),
								forced_by: None,
								role_changes: result.role_changes.clone(),
								nickname_change: result.nickname_change.clone(),
								relevant_connections: result.relevant_connections.clone()
							}]).await?;
						}
					},
					_ => ()
				}
			}
			tracker.send_logs(guild_id).await?;
		}
	}

	Ok(())
}

pub async fn member_update(event_data: &MemberUpdate) -> Result<()> {
	if !event_data.pending {
		let key = (event_data.guild_id, event_data.user.id);
		let pending = &PENDING_MEMBERS;
		let mut pending = pending.write().await;
		if pending.contains(&key) {
			pending.retain(|x| *x != key);

			let guild_id = &event_data.guild_id;
			let server_id = event_data.guild_id.to_string();
			if event_data.roles.is_empty() {
				let onboarding = GuildOnboarding::fetch(&server_id).await?;
				if !onboarding.enabled {
					let guild = Guild::fetch(&server_id).await?;
					match guild.verification_level {
						GuildVerificationLevel::High => {
							PENDING_VERIFICATION_TIMER.write().await.push((guild_id.clone(), event_data.user.id.clone(), SystemTime::now()));
							println!("added user to PENDING_VERIFICATION_TIMER");
							return Ok(());
						},
						_ => ()
					}
				}
			}

			let document = database::get_server_event_response_tree(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await?;
			if document.is_ready_for_stream() {
				let (mut stream, mut tracker) = document.into_stream(Variable::create_map([
					("member".into(), event_data.into())
				], None));
				while let Some((element, variables)) = stream.next().await {
					if process_element_for_member(&element, &variables, &mut tracker).await? { break }
				}
				tracker.send_logs(guild_id).await?;
			}
		}
	}

	Ok(())
}

pub async fn message_create(event_data: &MessageCreate) -> Result<()> {
	if let Some(guild_id) = &event_data.guild_id {
		let document = database::get_server_event_response_tree(guild_id, DocumentKind::MessageCreatedEvent).await?;
		if document.is_ready_for_stream() {
			let (mut stream, mut tracker) = document.into_stream(Variable::create_map([
				("member", Variable::from_partial_member(&event_data.author, event_data.member.as_ref().unwrap(), guild_id)),
				("message", event_data.into())
			], None));
			while let Some((element, variables)) = stream.next().await {
				if process_element_for_member(&element, &variables, &mut tracker).await? { break }
				match element.kind {
					ElementKind::Reply(data) => {
						if let Some(message) = data.reference.resolve(&variables) {
							create_channel_message(&event_data.channel_id.to_string(), ChannelMessage {
								content: Some(data.value),
								message_reference: Some(MessageReference {
									message_id: message.get("id").cast_str().into()
								}),
								..Default::default()
							}).await?;
						}
					},
					ElementKind::AddReaction(data) => {
						if let Some(message) = data.reference.resolve(&variables) {
							create_message_reaction(message.get("channel_id").cast_str(), message.get("id").cast_str(), data.value).await?;
						}
					},
					ElementKind::DeleteMessage(data) => {
						if let Some(message) = data.resolve(&variables) {
							let channel_id = message.get("channel_id").cast_str();
							delete_message(channel_id, message.get("id").cast_str()).await?;
							tracker.deleted_message(channel_id, message.get("author").get("id").cast_str());
						}
					},
					_ => ()
				}
			}
			tracker.send_logs(guild_id).await?;
		}
	}

	Ok(())
}