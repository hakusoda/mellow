use serde::{ Serialize, Deserialize };
use crate::{
	fetch::{ get_json, post_json, patch_json },
	interaction::{ Embed, InteractionResponseData },
	Result
};

pub const APP_ID: &str = env!("DISCORD_APP_ID");

pub async fn edit_original_response(token: impl Into<String>, payload: InteractionResponseData) -> Result<()> {
	patch_json(format!("https://discord.com/api/v10/webhooks/{}/{}/messages/@original", APP_ID, token.into()), payload).await
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

pub async fn modify_member(guild_id: String, user_id: String, payload: DiscordModifyMemberPayload) -> Result<()> {
	patch_json(format!("https://discord.com/api/v10/guilds/{guild_id}/members/{user_id}"), payload).await
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

pub async fn create_channel_message(channel_id: &String, payload: ChannelMessage) -> Result<()> {
	post_json(format!("https://discord.com/api/v10/channels/{channel_id}/messages"), payload).await
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct DiscordGuild {
	pub name: String,
	pub icon: Option<String>
}

pub async fn get_guild(guild_id: impl Into<String>) -> Result<DiscordGuild> {
	get_json(format!("https://discord.com/api/v10/guilds/{}", guild_id.into())).await
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

impl DiscordUser {
	pub fn display_name(&self) -> String {
		self.global_name.as_ref().unwrap_or(&self.username).clone()
	}
}

#[derive(Clone, Debug, Deserialize, Serialize)]
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

impl DiscordMember {
	pub fn id(&self) -> String {
		self.user.id.clone()
	}

	pub fn display_name(&self) -> String {
		self.nick.as_ref().map_or_else(|| self.user.display_name(), |x| x.clone())
	}
}

pub async fn get_member(guild_id: impl Into<String>, user_id: impl Into<String>) -> Result<DiscordMember> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/members/{}", guild_id.into(), user_id.into())).await
}

pub async fn get_members(guild_id: impl Into<String>) -> Result<Vec<DiscordMember>> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/members?limit=100", guild_id.into())).await
}

#[derive(Serialize, Deserialize, Debug)]
pub struct DiscordRole {
	pub id: String,
	pub name: String,
	pub icon: Option<String>,
	pub flags: u8,
	pub color: u32,
	pub hoist: bool,
	pub managed: bool,
	pub position: u8,
	pub mentionable: bool,
	pub permissions: String,
	pub unicode_emoji: Option<String>
}

pub async fn get_guild_roles(guild_id: impl Into<String>) -> Result<Vec<DiscordRole>> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/roles", guild_id.into())).await
}