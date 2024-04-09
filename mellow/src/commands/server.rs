use mellow_macros::command;
use twilight_model::application::interaction::Interaction;

use crate::{
	Result,
	discord::{ Guild, edit_original_response },
	database::{ DATABASE, server_exists, get_user_by_discord },
	interaction::InteractionResponseData,
	SlashResponse
};

#[tracing::instrument(skip_all)]
#[command(slash, no_dm, description = "Connect this server to mellow.", default_member_permissions = "0")]
pub async fn setup(interaction: Interaction) -> Result<SlashResponse> {
	let guild_id = interaction.guild_id.unwrap();
	Ok(if server_exists(&guild_id).await? {
		SlashResponse::Message {
			flags: Some(64),
			content: Some(format!("## Server already connected\nThis server is already connected to mellow, view it [here](https://hakumi.cafe/mellow/server/{guild_id})."))
		}
	} else {
		if let Some(user) = get_user_by_discord(&guild_id, &interaction.member.unwrap().user.unwrap().id).await? {
			SlashResponse::defer(interaction.token.clone(), Box::pin(async move {
				let guild = Guild::fetch(&guild_id).await.unwrap();
				DATABASE.from("mellow_servers")
					.insert(format!(r#"{{
						"id": "{guild_id}",
						"name": "{}",
						"creator_id": "{}",
						"avatar_url": "https://cdn.discordapp.com/icons/{guild_id}/{}.webp",
						"banner_url": {},
						"owner_user_id": "{}"
					}}"#, guild.name, user.user.id, guild.icon.unwrap_or("".into()), match guild.splash {
						Some(x) => format!("\"https://cdn.discordapp.com/splashes/{guild_id}/{x}.webp?size=1032\""),
						None => "null".into()
					}, user.user.id))
					.execute()
					.await?;

				edit_original_response(interaction.token, InteractionResponseData::ChannelMessageWithSource {
					flags: Some(64),
					embeds: None,
					content: Some(format!("## Server connected\nThis server is now connected to mellow!\nConfigure it online [here](https://hakumi.cafe/mellow/server/{guild_id}).\n\n*If you haven't already, to ensure that I can do what I do best, you may need to [position](https://support.discord.com/hc/en-us/articles/214836687-Role-Management-101#:~:text=drag%20to%20re-arrange%20roles) one of my roles at the very top of your server.*"))
				}).await
			}))
		} else {
			SlashResponse::Message {
				flags: Some(64),
				content: Some(format!("## Account not connected\nIt appears I do not recognise your wonderous face, you must be new!\n* Do you have a HAKUMI Account? If so, follow these [instructions](https://hakumi.cafe/docs/platform/account/connections), and then execute this command again.\n* If you're completely new, never heard of a HAKUMI, or even a measily marshmellow, simply [tap here](https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=setup_{guild_id})!"))
			}
		}
	})
}