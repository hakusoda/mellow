use tokio::time;
use mellow_macros::command;

use crate::{
	server::{ Log, LogKind, send_logs },
	discord::{ DiscordMember, get_members, edit_original_response },
	syncing::{ RoleChangeKind, sync_member, create_sign_up, get_connection_metadata },
	database::{ UserResponse, get_server, get_users_by_discord },
	interaction::{ Embed, EmbedField, InteractionPayload, InteractionResponseData },
	SlashResponse
};

pub async fn sync_with_token(user: UserResponse, member: DiscordMember, guild_id: &String, interaction_token: &String) {
	let server = get_server(guild_id).await;
	let metadata = get_connection_metadata(&vec![user.clone()], &server).await;

	let result = sync_member(Some(&user.user), &member, &server, &metadata, &mut None).await;
	edit_original_response(interaction_token, InteractionResponseData::ChannelMessageWithSource {
		flags: None,
		embeds: if result.role_changes.is_empty() { None } else { Some(vec![
			Embed {
				fields: Some(vec![
					EmbedField {
						name: "Role Changes".into(),
						value: format!("```diff\n{}```", result.role_changes.iter().map(|x| match x.kind {
							RoleChangeKind::Added => format!("+ {}", x.display_name),
							RoleChangeKind::Removed => format!("- {}", x.display_name)
						}).collect::<Vec<String>>().join("\n")),
						inline: None
					}
				]),
				..Default::default()
			}
		]) },
		content: Some(format!("{}{}", if result.profile_changed {
			format!("## Server Profile has been updated.\n{}",
				if result.role_changes.is_empty() { "" } else { "Your roles have been updated." }
			)
		} else {
			"## Server Profile is up-to-date.\nYour server profile is already up-to-date, no adjustments have been made.\n\nIf you were expecting a different result, you may need to wait a few minutes.".into()
		}, if server.actions.iter().all(|x| x.requirements.iter().all(|e| e.relevant_connection().map_or(true, |x| user.user.connections.iter().any(|e| x == e.connection.kind)))) { "".to_string() } else {
			format!("\n\n### You're missing connections\nYou haven't given this server access to all connections yet, change that [here](https://hakumi.cafe/mellow/server/{guild_id}/onboarding)!")
		}))
	}).await;

	if result.profile_changed {
		send_logs(&server, vec![Log {
			kind: LogKind::ServerProfileSync,
			data: serde_json::json!({
				"member": member,
				"role_changes": result.role_changes,
				"relevant_connections": result.relevant_connections
			})
		}]).await;
	}
}

#[command]
pub async fn sync(interaction: InteractionPayload) -> SlashResponse {
	let guild_id = interaction.guild_id.clone().unwrap();
	let member = interaction.member.unwrap();
	if let Some(user) = get_users_by_discord(vec![member.user.id.clone()], guild_id.clone()).await.into_iter().next() {
		tokio::spawn(async move {
			sync_with_token(user, member, &guild_id, &interaction.token).await;
		});
		return SlashResponse::DeferMessage;
	}

	create_sign_up(member.user.id, guild_id, interaction.token).await;
	SlashResponse::Message {
		flags: Some(64),
		content: Some(format!("## Hello, welcome to the server!\nYou appear to be new to mellow, this server uses mellow to sync member profiles with external services, such as Roblox.\nIf you would like to continue, please continue [here](https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv1%2Fauth%2Fcallback%2F0&response_type=code&scope=identify&state=mlw{}mlw), don't worry, it shouldn't take long!", interaction.guild_id.unwrap()))
	}
}

#[command]
pub async fn forcesyncall(interaction: InteractionPayload) -> SlashResponse {
	tokio::spawn(async move {
		let guild_id = interaction.guild_id.unwrap();
		let server = get_server(&guild_id).await;
		let members = get_members(&guild_id).await;
		let users = get_users_by_discord(members.iter().map(|x| x.user.id.clone()).collect(), guild_id).await;

		let metadata = get_connection_metadata(&users, &server).await;

		let mut logs: Vec<Log> = vec![];
		let mut total_synced = 0;
		let mut total_changed = 0;

		let mut guild_roles = None;
		for member in members {
			let result = sync_member(users.iter().find(|x| x.sub == member.user.id).map(|x| &x.user), &member, &server, &metadata, &mut guild_roles).await;
			if result.profile_changed {
				time::sleep(time::Duration::from_secs(1)).await;
				total_changed += 1;

				logs.push(Log {
					kind: LogKind::ServerProfileSync,
					data: serde_json::json!({
						"member": member,
						"role_changes": result.role_changes,
						"relevant_connections": result.relevant_connections
					})
				});
			}

			total_synced += 1;
		}

		edit_original_response(interaction.token, InteractionResponseData::ChannelMessageWithSource {
			flags: None,
			embeds: None,
			content: Some(format!("## Successfully synced {total_synced} profiles\n{total_changed} profile(s) in total were updated."))
		}).await;

		if !logs.is_empty() {
			send_logs(&server, logs).await;
		}
	});
	SlashResponse::DeferMessage
}