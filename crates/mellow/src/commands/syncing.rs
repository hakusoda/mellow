use dashmap::DashMap;
use mellow_cache::CACHE;
use mellow_macros::command;
use mellow_models::{
	hakumi::user::connection::ConnectionKind,
	mellow::ServerModel
};
use mellow_util::{
	hakuid::{
		marker::UserMarker as HakuUserMarker,
		HakuId
	},
	create_website_token,
	DISCORD_INTERACTION_CLIENT
};
use tokio::time;
use twilight_model::{
	id::{ marker::{ GuildMarker, UserMarker }, Id },
	application::interaction::InteractionData
};

use crate::{
	server::logging::{ ServerLog, send_logs },
	syncing::{
		sign_ups::create_sign_up,
		RoleChangeKind, SyncingInitiator, SyncingIssue, SyncMemberResult,
		sync_member, get_connection_metadata, sync_single_user
	},
	Result, Context, Interaction, CommandResponse,
	cast
};

#[tracing::instrument]
pub async fn sync_with_token(guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, member_id: Id<UserMarker>, interaction_token: &String, is_onboarding: bool, forced_by: Option<Id<UserMarker>>) -> Result<SyncMemberResult> {
	let initiator = match forced_by {
		Some(x) => SyncingInitiator::ForcedBy(x),
		None => SyncingInitiator::Manual
	};
	let result = sync_single_user(guild_id, user_id, member_id, initiator, None).await?;
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
	
	// we need localisation, my gosh 😭
	let is_forced = forced_by.is_some();
	let (pronoun, determiner, contraction) = if is_forced {
		(format!("<@{member_id}> has"), "Their", "they're")
	} else {
		("You have".into(), "Your", "you're")
	};
	let website_token = create_website_token(user_id)
		.await?;
	let content = format!("{}\n[<:gear_fill:1224667889592700950>  Your Server Preferences <:external_link:1225472071417729065>](<https://hakumi.cafe/mellow/server/{guild_id}/user_settings?mt={website_token}>)   •  [<:personraisedhand:1219234152709095424> Get Support](<https://discord.com/invite/rs3r4dQu9P>)",
		if result.profile_changed {
			format!("## {}\n```diff\n{}```",
				if result.issues.is_empty() {
					format!("<:check2circle:1219235152580837419>  {determiner} server profile has been updated.\n{}",
						if has_assigned_role && has_retracted_role {
							format!("{pronoun} been assigned and retracted roles, ...equality! o(>ω<)o")
						} else if has_assigned_role {
							format!("{pronoun} been assigned new roles, {}",
								if is_forced { "yippee!" } else { "hold them dearly to your heart! ♡(>ᴗ•)" }
							)
						} else {
							format!("{pronoun} been retracted some roles, that's either a good thing, or a bad thing! ┐(︶▽︶)┌")
						}
					)
				} else {
					format!("There was an issue while syncing your profile.\n{}",
						SyncingIssue::format_many(&result.issues, guild_id, user_id, &website_token)
							.await?
					)
				},
				result.role_changes
					.iter()
					.map(|x| match x.kind {
						RoleChangeKind::Added => format!("+ {}", x.display_name),
						RoleChangeKind::Removed => format!("- {}", x.display_name)
					})
					.collect::<Vec<String>>()
					.join("\n")
			)
		} else if !result.issues.is_empty() {
			format!("## There was an issue while syncing your profile.\n{}\n",
				SyncingIssue::format_many(&result.issues, guild_id, user_id, &website_token)
					.await?
			)
		} else {
			format!("## <:mellow_squircled:1225413361777508393>  {determiner} server profile is already up to par!\nAccording to my simulated brain, there's nothing to change here, {contraction} all set!\nIf you were expecting a *different* result, you may need to try again in a few minutes, apologies!\n")
		}
	);
	DISCORD_INTERACTION_CLIENT
		.update_response(interaction_token)
		.content(Some(&content))
		.await?;

	let mut server_logs: Vec<ServerLog> = vec![];
	if is_onboarding {
		server_logs.push(ServerLog::UserCompletedOnboarding {
			user_id: member_id
		});
	}

	if let Some(result_log) = result.create_log() {
		server_logs.push(result_log);
	}

	send_logs(guild_id, server_logs)
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
	if let Some(user_id) = CACHE.hakumi.user_by_discord(guild_id, member_id).await? {
		return Ok(CommandResponse::defer(
			interaction.token.clone(),
			Box::pin(async move {
				sync_with_token(guild_id, user_id, member_id, &interaction.token, false, None).await?;
				Ok(())
			})
		));
	}

	create_sign_up(guild_id, member_id, interaction.token).await;
	
	let guild = CACHE.discord
		.guild(guild_id)
		.await?;
	Ok(CommandResponse::ephemeral(
		format!("# <:waving_hand:1225409285203431565> <:mellow_squircled:1225413361777508393>  mellow says konnichiwa (hello)!\nWelcome to *{}*, before you can start syncing here, you need to get set up with mellow!\nWhenever you're ready, tap [here](<https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi-new.hakumi.cafe%2Fv1%2Fconnection_callback%2F0&response_type=code&scope=identify&state=mellow_new.{}>) to proceed, it won't take long!\n\n*fancy knowing what mellow is? read up on it [here](<https://hakumi.cafe/docs/mellow>)!*", guild.name, guild_id)
	))
}

fn server_not_found() -> Result<CommandResponse> {
	Ok(CommandResponse::ephemeral(
		"## <:niko_look_left:1227198516590411826>  Cannot sync member\nThis server hasn't been set up with mellow yet, if you're an administrator, execute the /setup command."
	))
}

fn forceful_disabled_response(guild_id: Id<GuildMarker>) -> Result<CommandResponse> {
	Ok(CommandResponse::ephemeral(
		format!("## <:niko_look_left:1227198516590411826>  Cannot sync member\nThis server has forceful syncing disabled, if you're a server manager...you may enable it [here](<https://hakumi.cafe/mellow/server/{guild_id}/settings/syncing>).")
	))
}

#[tracing::instrument(name = "commands::forcesync", skip_all)]
#[command(user, no_dm, rename = "Sync Profile", default_member_permissions = "268435456")]
pub async fn forcesync(_context: Context, interaction: Interaction) -> Result<CommandResponse> {
	// can we get so much higher (height)
	let guild_id = interaction.guild_id.unwrap();
	let Some(server) = CACHE.mellow.server(guild_id) else {
		return server_not_found();
	};
	if !server.allow_forced_syncing {
		return forceful_disabled_response(guild_id);
	}

	let resolved = cast!(interaction.data.unwrap(), InteractionData::ApplicationCommand).unwrap().resolved.unwrap();
	let member_id = resolved.members.into_iter().next().unwrap().0;
	if let Some(user_id) = CACHE.hakumi.user_by_discord(guild_id, member_id).await? {
		return Ok(CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
			sync_with_token(guild_id, user_id, member_id, &interaction.token, false, Some(interaction.user_id.unwrap())).await?;
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
	let Some(server) = CACHE.mellow.server(guild_id) else {
		return server_not_found();
	};
	if !server.allow_forced_syncing {
		return forceful_disabled_response(guild_id);
	}

	Ok(CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
		let user_ids = ServerModel::users(guild_id)
			.await?;
		let user_connection_ids = CACHE
			.hakumi
			.user_connections(&user_ids)
			.await?;
		let user_connections = CACHE
			.hakumi
			.connections(&user_connection_ids)
			.await?;
		let discord_user_ids = user_connections
			.iter()
			.filter_map(|x| if x.kind == ConnectionKind::Discord {
				Some(Id::new(x.sub.parse().unwrap()))
			} else { None })
			.collect();
		let mapped_user_ids: DashMap<Id<UserMarker>, HakuId<HakuUserMarker>> = DashMap::with_capacity(user_ids.len());
		for connection in user_connections {
			if connection.is_discord() {
				mapped_user_ids.insert(Id::new(connection.sub.parse().unwrap()), connection.user_id);
			}
		}

		let metadata = get_connection_metadata(guild_id, &user_ids)
			.await?;
		let members: Vec<_> = context
			.members(guild_id, discord_user_ids)
			.await?
			.into_iter()
			.map(|x| x.user_id)
			.collect();

		let mut logs: Vec<ServerLog> = vec![];
		let mut total_synced = 0;
		let mut total_changed = 0;
		for user_id in members {
			let result = sync_member(guild_id, mapped_user_ids.remove(&user_id).map(|x| x.1), user_id, SyncingInitiator::ForcedBy(interaction.user_id.unwrap()), &metadata)
				.await?;
			if let Some(result_log) = result.create_log() {
				logs.push(result_log);
			}

			if result.profile_changed {
				// sleep for one second to avoid hitting Discord ratelimit
				time::sleep(time::Duration::from_secs(1)).await;
				total_changed += 1;
			}

			total_synced += 1
		}

		DISCORD_INTERACTION_CLIENT
			.update_response(&interaction.token)
			.content(Some(&format!("## Successfully synced {total_synced} profiles\n{total_changed} profile(s) in total were updated.")))
			.await?;

		send_logs(guild_id, logs)
			.await?;
		Ok(())
	})))
}