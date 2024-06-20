use simd_json::json;
use mellow_macros::command;

use crate::{
	model::{
		discord::DISCORD_MODELS,
		hakumi::HAKUMI_MODELS
	},
	discord::INTERACTION,
	database::{ DATABASE, server_exists },
	Result, Context, Interaction, CommandResponse
};

#[tracing::instrument(skip_all)]
#[command(slash, no_dm, description = "Connect this server to mellow.", default_member_permissions = "0")]
pub async fn setup(_context: Context, interaction: Interaction) -> Result<CommandResponse> {
	let guild_id = interaction.guild_id.unwrap();
	Ok(if server_exists(&guild_id).await? {
		CommandResponse::ephemeral(
			format!("## Server already connected\nThis server is already connected to mellow, view it [here](https://hakumi.cafe/mellow/server/{guild_id}).")
		)
	} else if let Some(user) = HAKUMI_MODELS.user_by_discord(guild_id, interaction.user_id.unwrap()).await? {
		CommandResponse::defer(interaction.token.clone(), Box::pin(async move {
			let guild = DISCORD_MODELS.guild(guild_id).await?;
			DATABASE.from("mellow_servers")
				.insert(json!({
					"id": guild_id,
					"name": guild.name,
					"creator_id": user.id,
					"avatar_url": guild.icon.map(|x| format!("https://cdn.discordapp.com/icons/{guild_id}/{x}.webp")),
					"banner_url": guild.splash.map(|x| format!("https://cdn.discordapp.com/splashes/{guild_id}/{x}.webp?size=1032")),
					"owner_user_id": user.id
				}))?
				.await?;

			INTERACTION.update_response(&interaction.token)
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