use std::collections::HashMap;
use tokio_stream::StreamExt;
use twilight_model::gateway::payload::incoming::{ MemberAdd, MessageCreate };

use crate::{
	server::{
		logging::{ ServerLog, ProfileSyncKind, EventResponseResultMemberResult },
		Server
	},
	syncing::sync_single_user,
	discord::{ DiscordMember, ChannelMessage, MessageReference, ban_member, get_member, remove_member, create_channel_message },
	database,
	visual_scripting::{ Element, DocumentKind, ElementStream },
	Result
};

enum EventProcessorResult {
	MemberBanned,
	MemberKicked,
	MemberSynced,
	None
}

fn member_to_json(member: &DiscordMember) -> (String, serde_json::Value) {
	("member".into(), serde_json::json!({
		"id": member.id(),
		"username": member.user.username,
		"avatar_url": member.user.avatar.as_ref().map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", member.id())),
		"display_name": member.display_name()
	}))
}

pub async fn member_add(event_data: &MemberAdd) -> Result<()> {
	let user_id = event_data.user.id.to_string();
	let server_id = event_data.guild_id.to_string();

	let document = database::get_server_event_response_tree(&server_id, DocumentKind::MemberJoinEvent).await?;
	let definition = document.definition;
	if !definition.is_empty() {
		if let Some(user) = database::get_user_by_discord(&user_id, &server_id).await? {
			// TODO: this member get is pointless, replace with event_data.member
			let member = get_member(&server_id, &user_id).await?;
			let mut element_stream = ElementStream::new(definition, HashMap::from([
				member_to_json(&member)
			]));

			let mut processor_result = EventProcessorResult::None;
			while let Some(element) = element_stream.next().await {
				match element {
					Element::BanMember => {
						ban_member(&server_id, member.id()).await?;
						processor_result = EventProcessorResult::MemberBanned;
						break;
					},
					Element::KickMember => {
						remove_member(&server_id, member.id()).await?;
						processor_result = EventProcessorResult::MemberKicked;
						break;
					},
					Element::SyncMemberProfile => {
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
						processor_result = EventProcessorResult::MemberSynced;
						break;
					},
					_ => ()
				}
			}

			Server::fetch(server_id).await?.send_logs(vec![ServerLog::EventResponseResult {
				invoker: member.clone(),
				event_kind: document.name,
				member_result: match processor_result {
					EventProcessorResult::MemberBanned => EventResponseResultMemberResult::Banned,
					EventProcessorResult::MemberKicked => EventResponseResultMemberResult::Kicked,
					_ => EventResponseResultMemberResult::None
				}
			}]).await?;
		}
	}

	Ok(())
}

pub async fn message_create(event_data: &MessageCreate) -> Result<()> {
	let user_id = event_data.author.id.to_string();
	let server_id = event_data.guild_id.unwrap().to_string();

	let document = database::get_server_event_response_tree(&server_id, DocumentKind::MessageCreatedEvent).await?;
	let definition = document.definition;
	if !definition.is_empty() {
		let member = get_member(&server_id, &user_id).await?;
		let mut element_stream = ElementStream::new(definition, HashMap::from([
			member_to_json(&member),
			("message".into(), serde_json::json!({
				"content": event_data.content.clone()
			}))
		]));

		while let Some(element) = element_stream.next().await {
			match element {
				Element::BanMember => {
					ban_member(&server_id, member.id()).await?;
					break;
				},
				Element::KickMember => {
					remove_member(&server_id, member.id()).await?;
					break;
				},
				Element::Reply(text) => {
					create_channel_message(&event_data.channel_id.to_string(), ChannelMessage {
						content: Some(text.text),
						message_reference: Some(MessageReference {
							message_id: event_data.id.to_string()
						}),
						..Default::default()
					}).await?;
					break;
				},
				_ => ()
			}
		}
	}

	Ok(())
}