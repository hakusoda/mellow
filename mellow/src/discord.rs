use serde::{ Serialize, Deserialize };
use reqwest::{ header, Client };
use once_cell::sync::Lazy;

use crate::interaction::{ Embed, InteractionResponseData };

static APP_ID: Lazy<String> = Lazy::new(|| std::env::var("DISCORD_APP_ID").unwrap());
static CLIENT: Lazy<Client> = Lazy::new(||
	Client::builder()
		.default_headers({
			let mut headers = header::HeaderMap::new();
			headers.append("authorization", format!("Bot {}", std::env::var("DISCORD_TOKEN").unwrap()).parse().unwrap());
			headers
		})
		.build()
		.unwrap()
);

pub async fn edit_original_response(token: impl Into<String>, payload: InteractionResponseData) {
	CLIENT.patch(format!("https://discord.com/api/v10/webhooks/{}/{}/messages/@original", APP_ID.to_string(), token.into()))
		.body(serde_json::to_string(&payload).unwrap())
		.header("content-type", "application/json")
		.send()
		.await
		.unwrap();
}

#[derive(Serialize, Debug)]
pub struct DiscordModifyMemberPayload {
	#[serde(skip_serializing_if = "Option::is_none")]
	pub deaf: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub mute: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub nick: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub flags: Option<u8>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub roles: Option<Vec<String>>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub channel_id: Option<String>,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub communication_disabled_until: Option<String>
}

impl Default for DiscordModifyMemberPayload {
	fn default() -> Self {
		Self {
			deaf: None,
			mute: None,
			nick: None,
			flags: None,
			roles: None,
			channel_id: None,
			communication_disabled_until: None
		}
	}
}

pub async fn modify_member(guild_id: String, user_id: String, payload: DiscordModifyMemberPayload) {
	CLIENT.patch(format!("https://discord.com/api/v10/guilds/{guild_id}/members/{user_id}"))
		.body(serde_json::to_string(&payload).unwrap())
		.header("content-type", "application/json")
		.send()
		.await
		.unwrap();
}

#[derive(Serialize, Deserialize)]
pub struct ChannelMessage {
	pub embeds: Option<Vec<Embed>>,
	pub content: Option<String>
}

impl Default for ChannelMessage {
	fn default() -> Self {
		Self {
			embeds: None,
			content: None
		}
	}
}

pub async fn create_channel_message(channel_id: &String, payload: ChannelMessage) {
	CLIENT.post(format!("https://discord.com/api/v10/channels/{channel_id}/messages"))
		.body(serde_json::to_string(&payload).unwrap())
		.header("content-type", "application/json")
		.send()
		.await
		.unwrap();
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordUser {
	pub id: String,
	pub bot: Option<bool>,
	pub avatar: Option<String>,
	pub username: String,
	pub global_name: Option<String>,
	pub public_flags: u64,
	pub discriminator: String,
	pub avatar_decoration: Option<String>
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordMember {
	pub deaf: bool,
	pub mute: bool,
	pub nick: Option<String>,
	pub user: DiscordUser,
	pub roles: Vec<String>,
	pub avatar: Option<String>,
	pub pending: bool,
	pub joined_at: String,
	pub permissions: Option<String>
}

/*pub struct Ratelimit {
	pub wait_duration: Option<tokio::time::Duration>
}*/

pub async fn get_member(guild_id: impl Into<String>, user_id: impl Into<String>) -> DiscordMember {
	CLIENT.get(format!("https://discord.com/api/v10/guilds/{}/members/{}", guild_id.into(), user_id.into()))
		.send()
		.await
		.unwrap()
		.json()
		.await
		.unwrap()
}

pub async fn get_members(guild_id: impl Into<String>) -> Vec<DiscordMember> {
	let response = CLIENT.get(format!("https://discord.com/api/v10/guilds/{}/members?limit=100", guild_id.into()))
		.send()
		.await
		.unwrap();
	

	response.json()
		.await
		.unwrap()/*, Ratelimit {
			wait_duration: response.headers().get("x-ratelimit-remaining").and_then(|x| if x.to_str().unwrap() == "0" {
				response.headers().get("x-ratelimit-reset-after").map(|x| tokio::time::Duration::from_secs(x.to_str().unwrap().parse::<u64>().unwrap()))
			} else { None })
		})*/
}