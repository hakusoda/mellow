use tokio::time;
use mellow_macros::command;

use crate::{
	server::{ ServerLog, ProfileSyncKind },
	discord::{ DiscordMember, get_members, edit_original_response },
	syncing::{ RoleChangeKind, SyncMemberResult, sync_member, create_sign_up, get_connection_metadata, sync_single_user },
	database::{ UserResponse, get_server, get_user_by_discord, get_users_by_discord },
	interaction::{ Embed, EmbedField, InteractionPayload, InteractionResponseData },
	Result, SlashResponse
};

pub async fn sync_with_token(user: UserResponse, member: DiscordMember, guild_id: &String, interaction_token: &String) -> Result<SyncMemberResult> {
	let result = sync_single_user(&user, &member, guild_id, None).await?;
	let mut fields = vec![];
	if let Some(changes) = &result.nickname_change {
		fields.push(EmbedField {
			name: "Nickname changes".into(),
			value: format!("```diff{}{}```",
				changes.0.as_ref().map(|x| format!("\n- {x}")).unwrap_or("".into()),
				changes.1.as_ref().map(|x| format!("\n+ {x}")).unwrap_or("".into())
			),
			inline: None
		});
	}
	if !result.role_changes.is_empty() {
		fields.push(EmbedField {
			name: "Role changes".into(),
			value: format!("```diff\n{}```", result.role_changes.iter().map(|x| match x.kind {
				RoleChangeKind::Added => format!("+ {}", x.display_name),
				RoleChangeKind::Removed => format!("- {}", x.display_name)
			}).collect::<Vec<String>>().join("\n")),
			inline: None
		});
	}

	edit_original_response(interaction_token, InteractionResponseData::ChannelMessageWithSource {
		flags: None,
		embeds: if !fields.is_empty() { Some(vec![
			Embed {
				fields: Some(fields),
				..Default::default()
			}
		]) } else { None },
		content: Some(format!("{}{}\n\n[<:personbadge:1219233857786875925>  Change Connections](https://hakumi.cafe/mellow/server/{}/onboarding)  • [<:personraisedhand:1219234152709095424> Get Support](https://discord.com/invite/rs3r4dQu9P)", if result.profile_changed {
			format!("## <:check2circle:1219235152580837419>  Server Profile has been updated.\n{}",
				if result.role_changes.is_empty() { "" } else { "Your roles have been updated." }
			)
		} else {
			"## <:check2circle:1219235152580837419>  Server Profile is up-to-date.\nYour server profile is already up-to-date, no adjustments have been made.\nIf you were expecting a different result, you may need to wait a few minutes.".into()
		}, if result.server.actions.iter().all(|x| x.requirements.iter().all(|e| e.relevant_connection().map_or(true, |x| user.user.connections.iter().any(|e| x == e.connection.kind)))) { "".to_string() } else {
			format!("\n\n### You're missing connections\nYou haven't given this server access to all connections yet, change that [here](https://hakumi.cafe/mellow/server/{guild_id}/onboarding)!")
		}, guild_id))
	}).await?;

	if result.profile_changed {
		result.server.send_logs(vec![ServerLog::ServerProfileSync {
			kind: ProfileSyncKind::Default,
			member,
			forced_by: None,
			role_changes: result.role_changes.clone(),
			nickname_change: result.nickname_change.clone(),
			relevant_connections: result.relevant_connections.clone()
		}]).await?;
	}

	Ok(result)
}

// TODO: allow users to sync in dms via some sort of server selection
#[command(no_dm, description = "Sync your server profile. (may contain traces of burgers)")]
pub async fn sync(interaction: InteractionPayload) -> Result<SlashResponse> {
	let guild_id = interaction.guild_id.clone().unwrap();
	let member = interaction.member.unwrap();
	if let Some(user) = get_user_by_discord(member.id(), &guild_id).await? {
		return Ok(SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
			sync_with_token(user, member, &guild_id, &interaction.token).await?;
			Ok(())
		})));
	}

	create_sign_up(member.id(), guild_id, interaction.token).await;
	Ok(SlashResponse::Message {
		flags: Some(64),
		content: Some(format!("## Hello, welcome to the server!\nYou appear to be new to mellow, this server uses mellow to sync member profiles with external services, such as Roblox.\nIf you would like to continue, please continue [here](https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=sync_{}), don't worry, it shouldn't take long!", interaction.guild_id.unwrap()))
	})
}

#[command(no_dm, description = "Forcefully sync every member in the server.", default_member_permissions = "0")]
pub async fn forcesyncall(interaction: InteractionPayload) -> Result<SlashResponse> {
	Ok(SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
		let guild_id = interaction.guild_id.unwrap();
		
		let server = get_server(&guild_id).await?;
		let members = get_members(&guild_id).await?;
		
		let users = get_users_by_discord(members.iter().map(|x| x.id()).collect(), guild_id).await?;
		let metadata = get_connection_metadata(&users, &server).await?;

		let mut logs: Vec<ServerLog> = vec![];
		let mut total_synced = 0;
		let mut total_changed = 0;

		let mut guild_roles = None;
		for member in members {
			let result = sync_member(users.iter().find(|x| x.sub == member.id()).map(|x| &x.user), &member, &server, &metadata, &mut guild_roles).await?;
			if result.profile_changed {
				// sleep for one second to avoid hitting Discord ratelimit
				time::sleep(time::Duration::from_secs(1)).await;
				total_changed += 1;

				logs.push(ServerLog::ServerProfileSync {
					kind: ProfileSyncKind::Default,
					member,
					forced_by: interaction.member.clone(),
					role_changes: result.role_changes,
					nickname_change: result.nickname_change,
					relevant_connections: result.relevant_connections
				});
			}

			total_synced += 1;
		}

		edit_original_response(interaction.token, InteractionResponseData::ChannelMessageWithSource {
			flags: None,
			embeds: None,
			content: Some(format!("## Successfully synced {total_synced} profiles\n{total_changed} profile(s) in total were updated."))
		}).await?;

		server.send_logs(logs).await?;
		Ok(())
	})))
}