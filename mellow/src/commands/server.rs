use mellow_macros::command;

use crate::{
	discord::{ get_guild, edit_original_response },
	database::{ DATABASE, server_exists, get_users_by_discord },
	interaction::{ InteractionPayload, InteractionResponseData },
	SlashResponse
};

#[command(no_dm, description = "Connect this server to mellow.", default_member_permissions = "0")]
pub async fn setup(interaction: InteractionPayload) -> SlashResponse {
	let guild_id = interaction.guild_id.unwrap();
	if server_exists(&guild_id).await {
		SlashResponse::Message {
			flags: Some(64),
			content: Some(format!("## Server already connected\nThis server is already connected to mellow, view it [here](https://hakumi.cafe/mellow/server/{guild_id})."))
		}
	} else {
		if let Some(user) = get_users_by_discord(vec![interaction.member.unwrap().id()], guild_id.clone()).await.into_iter().next() {
			tokio::spawn(async move {
				let guild = get_guild(&guild_id).await;
				DATABASE.from("mellow_servers")
					.insert(format!(r#"{{
						"id": "{guild_id}",
						"name": "{}",
						"creator_id": "{}",
						"avatar_url": "https://cdn.discordapp.com/icons/{guild_id}/{}.webp",
						"owner_user_id": "{}"
					}}"#, guild.name, user.sub, guild.icon.unwrap_or("".into()), user.sub))
					.execute()
					.await
					.unwrap();

				edit_original_response(interaction.token, InteractionResponseData::ChannelMessageWithSource {
					flags: Some(64),
					embeds: None,
					content: Some(format!("## Server connected\nThis server is now connected to mellow!\nConfigure it online [here](https://hakumi.cafe/mellow/server/{guild_id}).\n\n*If you haven't already, to ensure that I can do what I do best, you may need to [position](https://support.discord.com/hc/en-us/articles/214836687-Role-Management-101#:~:text=drag%20to%20re-arrange%20roles) one of my roles at the very top of your server.*"))
				}).await;
			});
			SlashResponse::DeferMessage
		} else {
			SlashResponse::Message {
				flags: Some(64),
				content: Some(format!("## Account not connected\nIt appears I do not recognise your wonderous face, you must be new!\n* Do you have a HAKUMI Account? If so, follow these [instructions](https://hakumi.cafe/docs/platform/account/connections), and then execute this command again.\n* If you're completely new, never heard of a HAKUMI, or even a measily marshmellow, simply [tap here](https://discord.com/api/oauth2/authorize?client_id=1068554282481229885&redirect_uri=https%3A%2F%2Fapi.hakumi.cafe%2Fv0%2Fauth%2Fcallback%2Fmellow&response_type=code&scope=identify&state=setup_{guild_id})!"))
			}
		}
	}
}