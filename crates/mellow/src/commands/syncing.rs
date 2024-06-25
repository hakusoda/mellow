use tokio::time;
use mellow_macros::command;
use twilight_model::{
	id::{ marker::{ GuildMarker, UserMarker }, Id },
	application::interaction::InteractionData
};

use crate::{
	model::{
		discord::DISCORD_MODELS,
		hakumi::{
			id::{ marker::UserMarker as HakuUserMarker, HakuId },
			user::connection::ConnectionKind,
			HAKUMI_MODELS
		},
		mellow::MELLOW_MODELS
	},
	server::logging::{ ServerLog, ProfileSyncKind },
	discord::INTERACTION,
	syncing::{
		sign_ups::create_sign_up,
		MemberStatus, RoleChangeKind, SyncMemberResult,
		sync_member, get_connection_metadata, sync_single_user
	},
	Result, Context, Interaction, CommandResponse,
	cast
};

#[tracing::instrument]
pub async fn sync_with_token(guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, member_id: Id<UserMarker>, interaction_token: &String, is_onboarding: bool, forced_by: Option<Id<UserMarker>>) -> Result<SyncMemberResult> {
	let result = sync_single_user(guild_id, user_id, member_id, None).await?;
	let mut has_assigned_role = false;
	let mut has_retracted_role = false;
	for item in result.role_changes.iter() {
		match item.kind {
			RoleChangeKind::Added => has_assigned_role = true,
			RoleChangeKind::Removed => has_retracted_role = true
		}
		if has_assigned_role && has_retracted_role {
			break;
		}
	}

	let user2 = DISCORD_MODELS
		.user(member_id)
		.await?;
	let is_forced = forced_by.is_some();
	let (pronoun, determiner, contraction) = if is_forced { ("They", "Their", "they're") } else { ("You", "Your", "you're") };
	INTERACTION.update_response(interaction_token)
		.content(Some(&format!("{}{}\n[<:gear_fill:1224667889592700950>  Your Server Preferences <:external_link:1225472071417729065>](https://hakumi.cafe/mellow/server/{}/user_settings)   •  [<:personraisedhand:1219234152709095424> Get Support](https://discord.com/invite/rs3r4dQu9P)", if result.profile_changed {
			format!("## <:check2circle:1219235152580837419>  {determiner} server profile has been updated.\n{}```diff\n{}```",
				if has_assigned_role && has_retracted_role {
					format!("{pronoun} have been assigned and retracted roles, ...equality! o(>ω<)o")
				} else if has_assigned_role {
					format!("{pronoun} have been assigned new roles, {}",
						if is_forced { "yippee!" } else { "hold them dearly to your heart! ♡(>ᴗ•)" }
					)
				} else {
					format!("Some of {} roles were retracted, that's either a good thing, or a bad thing! ┐(︶▽︶)┌", determiner.to_lowercase())
				},
				result.role_changes.iter().map(|x| match x.kind {
					RoleChangeKind::Added => format!("+ {}", x.display_name),
					RoleChangeKind::Removed => format!("- {}", x.display_name)
				}).collect::<Vec<String>>().join("\n")
			)
		} else {
			format!("## <:mellow_squircled:1225413361777508393>  {determiner} server profile is already up to par!\nAccording to my simulated brain, there's nothing to change here, {contraction} all set!\nIf you were expecting a *different* result, you may need to try again in a few minutes, apologies!\n")
		}, if result.is_missing_connections {
			if is_forced {
				format!("\n***by the way...** {} hasn't yet connected all platforms this server utilises.*\n", user2.display_name())
			} else {
				format!("\n### You're missing connections\nYou haven't given this server access to all connections yet, change that [here](https://hakumi.cafe/mellow/server/{}/user_settings)!\n", guild_id)
			}
		} else { "".into() }, guild_id)))
		.await?;

	let mut server_logs: Vec<ServerLog> = vec![];
	if is_onboarding {
		server_logs.push(ServerLog::UserCompletedOnboarding {
			user_id: member_id
		});
	}

	if result.profile_changed || result.member_status.removed() {
		server_logs.push(ServerLog::ServerProfileSync {
			kind: match result.member_status {
				MemberStatus::Ok => ProfileSyncKind::Default,
				MemberStatus::Banned => ProfileSyncKind::Banned,
				MemberStatus::Kicked => ProfileSyncKind::Kicked
			},
			user_id: member_id,
			forced_by,
			role_changes: result.role_changes.clone(),
			nickname_change: result.nickname_change.clone(),
			relevant_connections: result.relevant_connections.clone()
		});
	}

	MELLOW_MODELS.server(result.server_id)
		.await?
		.send_logs(server_logs)
		.await?;

	Ok(result)
}

// TODO: allow users to sync in dms via some sort of server selection
#[tracing::instrument(name = "commands::sync", skip_all)]
#[command(slash, no_dm, description = "Sync your server profile. (may contain traces of burgers)")]
pub async fn sync(_context: Context, interaction: Interaction) -> Result<CommandResponse> {
	let member = interaction.member().await?.unwrap();
	let guild_id = interaction.guild_id.unwrap();
	let member_id = member.user_id;
	if let Some(user_id) = HAKUMI_MODELS.user_by_discord(guild_id, member_id).await? {
		return Ok(CommandResponse::defer(
			interaction.token.clone(),
			Box::pin(async move {
				sync_with_token(guild_id, *user_id, member_id, &interaction.token, false, None).await?;
				Ok(())
			})
		));
	}

	create_sign_up(guild_id, member_id, interaction.token).await;
	
	let guild = DISCORD_MODELS.guild(guild_id).await?;
	Ok(CommandResponse::ephemeral(
		format!("# <:waving_hand:1225409285203431565> <:mellow_squircled:1225413361777508393>  mellow says konnichiwa (hello)!\nWelcome to *{}*, before you can start syncing here, you need to get set up with mellow!\nWhenever you're ready, tap [here](<https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=sync.{}>) to proceed, it won't take long!\n\n*fancy knowing what mellow is? read up on it [here](<https://hakumi.cafe/docs/mellow>)!*", guild.name, guild_id)
	))
}

fn forceful_disabled_response(guild_id: Id<GuildMarker>) -> Result<CommandResponse> {
	Ok(CommandResponse::ephemeral(
		format!("## <:niko_look_left:1227198516590411826>  Cannot sync member\nThis server has forceful syncing disabled, if you're a server manager...you may enable it [here](https://hakumi.cafe/mellow/server/{guild_id}/settings/syncing).")
	))
}

#[tracing::instrument(name = "commands::forcesync", skip_all)]
#[command(user, no_dm, rename = "Sync Profile", default_member_permissions = "268435456")]
pub async fn forcesync(_context: Context, interaction: Interaction) -> Result<CommandResponse> {
	// can we get so much higher (height)
	let guild_id = interaction.guild_id.unwrap();
	let server = MELLOW_MODELS.server(guild_id).await?;
	if !server.allow_forced_syncing {
		return forceful_disabled_response(guild_id);
	}

	let resolved = cast!(interaction.data.unwrap(), InteractionData::ApplicationCommand).unwrap().resolved.unwrap();
	let member_id = resolved.members.into_iter().next().unwrap().0;
	if let Some(user_id) = HAKUMI_MODELS.user_by_discord(guild_id, member_id).await? {
		return Ok(CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
			sync_with_token(guild_id, *user_id, member_id, &interaction.token, false, Some(interaction.user_id.unwrap())).await?;
			Ok(())
		})));
	}
	
	Ok(CommandResponse::ephemeral(
		format!("## <:niko_look_left:1227198516590411826>  Cannot sync member\n<@{member_id}> has not yet been set up with mellow... ┐(￣ヘ￣)┌")
	))
}

#[tracing::instrument(name = "commands::forcesyncall", skip_all)]
#[command(slash, no_dm, description = "Forcefully sync every member in this server.", default_member_permissions = "0")]
pub async fn forcesyncall(context: Context, interaction: Interaction) -> Result<CommandResponse> {
	let guild_id = interaction.guild_id.unwrap();
	let server = MELLOW_MODELS.server(guild_id).await?;
	if !server.allow_forced_syncing {
		return forceful_disabled_response(guild_id);
	}

	Ok(CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
		let users = server.users(guild_id).await?;
		let user_ids = users
			.iter()
			.flat_map(|x| x.connections
				.iter()
				.filter_map(|x| if x.kind == ConnectionKind::Discord {
					Some(Id::new(x.sub.parse().unwrap()))
				} else { None })
			)
			.collect();
		let members = context.members(guild_id, user_ids).await?;
		let metadata = get_connection_metadata(guild_id, &users.iter().map(|x| x.id).collect()).await?;

		let mut logs: Vec<ServerLog> = vec![];
		let mut total_synced = 0;
		let mut total_changed = 0;
		for member in members {
			let string_id = member.user_id.to_string();

			let result = sync_member(guild_id, users.iter().find(|x| x.has_connection(&string_id, ConnectionKind::Discord)).map(|x| x.id), member.user_id, &metadata).await?;
			if result.profile_changed {
				// sleep for one second to avoid hitting Discord ratelimit
				time::sleep(time::Duration::from_secs(1)).await;
				total_changed += 1;

				logs.push(ServerLog::ServerProfileSync {
					kind: ProfileSyncKind::Default,
					user_id: member.user_id,
					forced_by: interaction.user_id,
					role_changes: result.role_changes,
					nickname_change: result.nickname_change,
					relevant_connections: result.relevant_connections
				});
			}

			total_synced += 1
		}

		INTERACTION.update_response(&interaction.token)
			.content(Some(&format!("## Successfully synced {total_synced} profiles\n{total_changed} profile(s) in total were updated.")))
			.await?;

		server.send_logs(logs).await?;
		Ok(())
	})))
}