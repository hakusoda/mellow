use mellow_cache::CACHE;
use mellow_macros::command;
use mellow_models::mellow::ServerModel;
use mellow_util::{ DISCORD_INTERACTION_CLIENT, PG_POOL };
use std::pin::Pin;
use twilight_model::id::Id;

use crate::{
	Result, Context, Interaction, CommandResponse
};

#[tracing::instrument(skip_all)]
#[command(slash, no_dm, description = "Connect this server to mellow.", default_member_permissions = "0")]
pub async fn setup(_context: Context, interaction: Interaction) -> Result<CommandResponse> {
	let guild_id = interaction.guild_id.unwrap();
	Ok(if CACHE.mellow.servers.contains_key(&guild_id) {
		CommandResponse::ephemeral(
			format!("## Server already connected\nThis server is already connected to mellow, view it [here](https://hakumi.cafe/mellow/server/{guild_id}).")
		)
	} else if let Some(user_id) = CACHE.hakumi.user_by_discord(guild_id, interaction.user_id.unwrap()).await? {
		CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
			let guild = CACHE.discord
				.guild(guild_id)
				.await?;

			let record = sqlx::query!(
				"
				INSERT INTO mellow_servers (id, name, creator_id, owner_user_id, avatar_url, banner_url)
				VALUES ($1, $2, $3, $3, $4, $5)
				RETURNING logging_types, default_nickname, logging_channel_id, allow_forced_syncing
				",
				guild_id.get() as i64,
				guild.name,
				user_id.value,
				guild.icon.map(|x| format!("https://cdn.discordapp.com/icons/{guild_id}/{x}.webp")),
				guild.splash.map(|x| format!("https://cdn.discordapp.com/splashes/{guild_id}/{x}.webp?size=1032"))
			)
				.fetch_one(&*Pin::static_ref(&PG_POOL).await)
				.await?;
			CACHE
				.mellow
				.servers
				.insert(guild_id, ServerModel {
					id: guild_id,
					logging_types: record.logging_types as u8,
					default_nickname: record.default_nickname,
					logging_channel_id: record.logging_channel_id.map(|x| Id::new(x as u64)),
					allow_forced_syncing: record.allow_forced_syncing
				});

			DISCORD_INTERACTION_CLIENT
				.update_response(&interaction.token)
				.content(Some(&format!("## Server connected\nThis server is now connected to mellow!\nConfigure it online [here](https://hakumi.cafe/mellow/server/{guild_id}).\n\n*If you haven't already, to ensure that I can do what I do best, you may need to [position](https://support.discord.com/hc/en-us/articles/214836687-Role-Management-101#:~:text=drag%20to%20re-arrange%20roles) one of my roles at the very top of your server.*")))
				.await?;
			Ok(())
		}))
	} else {
		CommandResponse::ephemeral(
			format!("## Account not connected\nIt appears I do not recognise your wondrous face, you must be new!\n* Do you have a HAKUMI Account? If so, follow these [instructions](https://hakumi.cafe/docs/platform/account/connections), and then execute this command again.\n* If you're completely new, never heard of a HAKUMI, or even a measily marshmellow, simply [tap here](https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=setup_{guild_id})!")
		)
	})
}