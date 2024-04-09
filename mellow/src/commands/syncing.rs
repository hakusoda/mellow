use tokio::time;
use mellow_macros::command;
use twilight_model::{
	id::{
		marker::GuildMarker,
		Id
	},
	guild::PartialMember,
	application::interaction::{ Interaction, InteractionData }
};

use crate::{
	util::member_into_partial,
	traits::{ Partial, QuickId, DisplayName },
	server::{
		logging::{ ServerLog, ProfileSyncKind },
		Server,
	},
	discord::{ Guild, get_members, edit_original_response },
	syncing::{
		sign_ups::create_sign_up,
		RoleChangeKind, SyncMemberResult,
		sync_member, get_connection_metadata, sync_single_user
	},
	database::{ UserResponse, get_user_by_discord, get_users_by_discord },
	interaction::InteractionResponseData,
	Result, SlashResponse,
	cast
};

#[tracing::instrument(skip(user, member))]
pub async fn sync_with_token(user: UserResponse, member: PartialMember, guild_id: &Id<GuildMarker>, interaction_token: &String, is_onboarding: bool, forced_by: Option<PartialMember>) -> Result<SyncMemberResult> {
	let result = sync_single_user(&user, &member, &guild_id, None).await?;
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

	let is_forced = forced_by.is_some();
	let (pronoun, determiner, contraction) = if is_forced { ("They", "Their", "they're") } else { ("You", "Your", "you're") };
	edit_original_response(interaction_token, InteractionResponseData::ChannelMessageWithSource {
		flags: None,
		embeds: None,
		content: Some(format!("{}{}\n[<:gear_fill:1224667889592700950>  Your Server Preferences <:external_link:1225472071417729065>](https://hakumi.cafe/mellow/server/{}/user_settings)   •  [<:personraisedhand:1219234152709095424> Get Support](https://discord.com/invite/rs3r4dQu9P)", if result.profile_changed {
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
		}, if result.server.actions.iter().all(|x| x.requirements.iter().all(|e| e.relevant_connection().map_or(true, |x| user.user.server_connections().into_iter().any(|e| x == e.kind)))) { "".to_string() } else {
			if is_forced {
				format!("\n***by the way...** {} hasn't yet connected all platforms this server utilises.*\n", member.display_name())
			} else {
				format!("\n### You're missing connections\nYou haven't given this server access to all connections yet, change that [here](https://hakumi.cafe/mellow/server/{guild_id}/user_settings)!\n")
			}
		}, guild_id))
	}).await?;

	let mut server_logs: Vec<ServerLog> = vec![];
	if is_onboarding {
		server_logs.push(ServerLog::UserCompletedOnboarding {
			member: member.clone()
		});
	}

	if result.profile_changed {
		server_logs.push(ServerLog::ServerProfileSync {
			kind: ProfileSyncKind::Default,
			member,
			forced_by,
			role_changes: result.role_changes.clone(),
			nickname_change: result.nickname_change.clone(),
			relevant_connections: result.relevant_connections.clone()
		});
	}

	result.server.send_logs(server_logs).await?;

	Ok(result)
}

// TODO: allow users to sync in dms via some sort of server selection
#[tracing::instrument(skip_all)]
#[command(slash, no_dm, description = "Sync your server profile. (may contain traces of burgers)")]
pub async fn sync(interaction: Interaction) -> Result<SlashResponse> {
	let member = interaction.member.unwrap();
	let user_id = member.id();
	let guild_id = interaction.guild_id.unwrap();
	if let Some(user) = get_user_by_discord(&guild_id, user_id).await? {
		return Ok(SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
			sync_with_token(user, member, &guild_id, &interaction.token, false, None).await?;
			Ok(())
		})));
	}

	create_sign_up(guild_id, *user_id, interaction.token).await;
	
	let guild = Guild::fetch(&guild_id).await?;
	Ok(SlashResponse::Message {
		flags: Some(64),
		content: Some(format!("# <:waving_hand:1225409285203431565> <:mellow_squircled:1225413361777508393>  mellow says konnichiwa (hello)!\nWelcome to *{}*, before you can start syncing here, you need to get set up with mellow!\nWhenever you're ready, tap [here](<https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=sync.{}>) to proceed, it won't take long!\n\n*fancy knowing what mellow is? read up on it [here](<https://hakumi.cafe/docs/mellow>)!*", guild.name, guild_id))
	})
}

#[tracing::instrument(skip_all)]
#[command(user, no_dm, rename = "Sync Profile", default_member_permissions = "268435456")]
pub async fn forcesync(interaction: Interaction) -> Result<SlashResponse> {
	// can we get so much higher (height)
	let guild_id = interaction.guild_id.unwrap();
	let resolved = cast!(interaction.data.unwrap(), InteractionData::ApplicationCommand).unwrap().resolved.unwrap();
	let (user_id, member) = resolved.members.into_iter().next().unwrap();
	let mut member = member.partial();
	member.user = Some(resolved.users.into_iter().next().unwrap().1);
	
	if let Some(user) = get_user_by_discord(&guild_id, &user_id).await? {
		return Ok(SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
			sync_with_token(user, member, &guild_id, &interaction.token, false, Some(interaction.member.unwrap())).await?;
			Ok(())
		})));
	}
	
	Ok(SlashResponse::Message {
		flags: Some(64),
		content: Some(format!("## <:niko_look_left:1227198516590411826>  Cannot sync member\n<@{user_id}> has not yet been set up with mellow... ┐(￣ヘ￣)┌"))
	})
}

#[tracing::instrument(skip_all)]
#[command(slash, no_dm, description = "Forcefully sync every member in this server.", default_member_permissions = "0")]
pub async fn forcesyncall(interaction: Interaction) -> Result<SlashResponse> {
	Ok(SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
		let guild_id = interaction.guild_id.unwrap();
		
		let server = Server::fetch(&guild_id).await?;
		let members = get_members(&guild_id).await?;
		
		let users = get_users_by_discord(&guild_id, members.iter().map(|x| &x.user.id).collect()).await?;
		let metadata = get_connection_metadata(&users, &server).await?;

		let mut logs: Vec<ServerLog> = vec![];
		let mut total_synced = 0;
		let mut total_changed = 0;

		let mut guild_roles = None;
		for member in members {
			let partial = member_into_partial(member.clone());
			let string_id = member.user.id.to_string();

			let result = sync_member(users.iter().find(|x| x.sub == string_id).map(|x| &x.user), &partial, &server, &metadata, &mut guild_roles).await?;
			if result.profile_changed {
				// sleep for one second to avoid hitting Discord ratelimit
				time::sleep(time::Duration::from_secs(1)).await;
				total_changed += 1;

				logs.push(ServerLog::ServerProfileSync {
					kind: ProfileSyncKind::Default,
					member: partial,
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