use std::collections::HashMap;
use tokio::sync::RwLock;
use tokio_stream::StreamExt;
use twilight_model::gateway::payload::incoming::{ MemberAdd, MemberUpdate, MessageCreate };

use crate::{
	server::logging::{ ServerLog, ProfileSyncKind },
	syncing::sync_single_user,
	discord::{
		ChannelMessage, MessageReference,
		ban_member, get_member, remove_member, assign_member_role, create_channel_message, create_message_reaction
	},
	database,
	visual_scripting::{ Element, Variable, ElementKind, DocumentKind, ElementStream },
	Result,
	cast
};

async fn process_element_for_member(element: &Element, variables: &Variable, server_id: &str) -> Result<bool> {
	Ok(match &element.kind {
		ElementKind::BanMember(reference) => {
			if let Some(member) = reference.resolve(&variables).and_then(|x| cast!(x, Variable::Member)) {
				ban_member(server_id, member.id).await?;
				true
			} else { false }
		},
		ElementKind::KickMember(reference) => {
			if let Some(member) = reference.resolve(&variables).and_then(|x| cast!(x, Variable::Member)) {
				remove_member(server_id, member.id).await?;
				true
			} else { false }
		},
		ElementKind::AssignRoleToMember(data) => {
			if let Some(member) = data.reference.resolve(&variables).and_then(|x| cast!(x, Variable::Member)) {
				assign_member_role(server_id, member.id, &data.value).await?;
				true
			} else { false }
		},
		_ => false
	})
}

static PENDING_MEMBERS: RwLock<Vec<(String, String)>> = RwLock::const_new(vec![]);

pub async fn member_add(event_data: &MemberAdd) -> Result<()> {
	let user_id = event_data.user.id.to_string();
	let server_id = event_data.guild_id.to_string();
	if event_data.member.pending {
		PENDING_MEMBERS.write().await.push((server_id.clone(), user_id.clone()));
	}

	let document = database::get_server_event_response_tree(&server_id, DocumentKind::MemberJoinEvent).await?;
	let definition = document.definition;
	if !definition.is_empty() {
		if let Some(user) = database::get_user_by_discord(&user_id, &server_id).await? {
			// TODO: this member get is pointless, replace with event_data.member
			let member = get_member(&server_id, &user_id).await?;
			let mut element_stream = ElementStream::new(definition, Variable::Map(HashMap::from([
				("member".into(), member.clone().into())
			])));

			while let Some((element, variables)) = element_stream.next().await {
				if process_element_for_member(&element, &variables, &server_id).await? { break }
				match element.kind {
					ElementKind::SyncMember => {
						let result = sync_single_user(&user, &member, &server_id, None).await?;
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
		}
	}

	Ok(())
}

pub async fn member_update(event_data: &MemberUpdate) -> Result<()> {
	if !event_data.pending {
		let key = (event_data.guild_id.to_string(), event_data.user.id.to_string());
		let pending = &PENDING_MEMBERS;
		let mut pending = pending.write().await;
		if pending.contains(&key) {
			pending.retain(|x| *x != key);

			let user_id = event_data.user.id.to_string();
			let server_id = event_data.guild_id.to_string();

			let document = database::get_server_event_response_tree(&server_id, DocumentKind::MemberCompletedOnboardingEvent).await?;
			let definition = document.definition;
			if !definition.is_empty() {
				// TODO: this member get is pointless, replace with event_data.member
				let member = get_member(&server_id, &user_id).await?;
				let mut element_stream = ElementStream::new(definition, Variable::Map(HashMap::from([
					("member".into(), member.clone().into())
				])));

				while let Some((element, variables)) = element_stream.next().await {
					if process_element_for_member(&element, &variables, &server_id).await? { break }
				}
			}
		}
	}

	Ok(())
}

pub async fn message_create(event_data: &MessageCreate) -> Result<()> {
	let server_id = event_data.guild_id.unwrap().to_string();

	let document = database::get_server_event_response_tree(&server_id, DocumentKind::MessageCreatedEvent).await?;
	let definition = document.definition;
	if !definition.is_empty() {
		let author = &event_data.author;
		let mut element_stream = ElementStream::new(definition, Variable::create_map([
			("member", author.clone().into()),
			("message", event_data.into())
		]));

		while let Some((element, variables)) = element_stream.next().await {
			if process_element_for_member(&element, &variables, &server_id).await? { break }
			match element.kind {
				ElementKind::Reply(data) => {
					if let Some(message) = data.reference.resolve(&variables).and_then(|x| cast!(x, Variable::Message)) {
						create_channel_message(&event_data.channel_id.to_string(), ChannelMessage {
							content: Some(data.value),
							message_reference: Some(MessageReference {
								message_id: message.id
							}),
							..Default::default()
						}).await?;
					}
				},
				ElementKind::AddReaction(data) => {
					if let Some(message) = data.reference.resolve(&variables).and_then(|x| cast!(x, Variable::Message)) {
						create_message_reaction(message.channel_id, message.id, data.value).await?;
					}
				},
				_ => ()
			}
		}
	}

	Ok(())
}