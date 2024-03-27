use std::collections::HashMap;
use tokio_stream::StreamExt;
use twilight_model::gateway::payload::incoming::MemberAdd;

use crate::{
	server::{ ServerLog, ProfileSyncKind, EventResponseResultMemberResult },
	syncing::sync_single_user,
	discord::{ ban_member, get_member, remove_member },
	database,
	visual_scripting::{ Element, ElementStream },
	Result
};

enum EventProcessorResult {
	MemberBanned,
	MemberKicked,
	MemberSynced,
	None
}

pub async fn member_add(event_data: &MemberAdd) -> Result<()> {
	let user_id = event_data.user.id.to_string();
	let server_id = event_data.guild_id.to_string();
	let response_tree = database::get_server_event_response_tree(&server_id, "member_join").await?;
	if !response_tree.is_empty() {
		if let Some(user) = database::get_user_by_discord(&user_id, &server_id).await? {
			let member = get_member(&server_id, &user_id).await?;
			let mut element_stream = ElementStream::new(response_tree, HashMap::from([
				("member".into(), serde_json::json!({
					"id": member.id(),
					"username": member.user.username,
					"avatar_url": member.user.avatar.as_ref().map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", member.id())),
					"display_name": member.display_name()
				}))
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

			database::get_server(server_id).await?.send_logs(vec![ServerLog::EventResponseResult {
				invoker: member.clone(),
				event_kind: "member_join".into(),
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