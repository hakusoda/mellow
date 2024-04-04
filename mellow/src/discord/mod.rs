use serde::{ Serialize, Deserialize };
use reqwest::Method;
use tracing::{ Instrument, info_span };
use serde_repr::{ Serialize_repr, Deserialize_repr };
use twilight_model::{
	id::{
		marker::{ RoleMarker, UserMarker, GuildMarker },
		Id
	},
	guild::{ Role, Member }
};
use percent_encoding::{ NON_ALPHANUMERIC, utf8_percent_encode };

use crate::{
	cache::CACHES,
	fetch::{ get_json, post_json, fetch_json, patch_json },
	interaction::{ Embed, InteractionResponseData },
	Result
};

pub mod gateway;

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
	pub roles: Option<Vec<Id<RoleMarker>>>,
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

pub async fn modify_member(guild_id: &Id<GuildMarker>, user_id: &Id<UserMarker>, payload: DiscordModifyMemberPayload) -> Result<()> {
	patch_json(format!("https://discord.com/api/v10/guilds/{guild_id}/members/{user_id}"), payload).await
}

pub async fn assign_member_role(guild_id: impl Into<String>, user_id: impl Into<String>, role_id: impl Into<String>) -> Result<()> {
	fetch_json(format!("https://discord.com/api/v10/guilds/{}/members/{}/roles/{}", guild_id.into(), user_id.into(), role_id.into()), Some(Method::PUT), None, None).await
}

pub async fn ban_member(guild_id: impl Into<String>, user_id: impl Into<String>) -> Result<()> {
	fetch_json(format!("https://discord.com/api/v10/guilds/{}/bans/{}", guild_id.into(), user_id.into()), Some(Method::PUT), None, None).await
}

pub async fn remove_member(guild_id: impl Into<String>, user_id: impl Into<String>) -> Result<()> {
	fetch_json(format!("https://discord.com/api/v10/guilds/{}/members/{}", guild_id.into(), user_id.into()), Some(Method::DELETE), None, None).await
}

#[derive(Serialize, Deserialize)]
pub struct ChannelMessage {
	pub embeds: Option<Vec<Embed>>,
	pub content: Option<String>,
	pub message_reference: Option<MessageReference>
}

#[derive(Serialize, Deserialize)]
pub struct MessageReference {
	pub message_id: String
}

impl Default for ChannelMessage {
	fn default() -> Self {
		Self {
			embeds: None,
			content: None,
			message_reference: None
		}
	}
}

pub async fn create_channel_message(channel_id: &String, payload: ChannelMessage) -> Result<()> {
	post_json(format!("https://discord.com/api/v10/channels/{channel_id}/messages"), payload).await
}

pub async fn create_message_reaction(channel_id: impl Into<String>, message_id: impl Into<String>, emoji: impl Into<String>) -> Result<()> {
	fetch_json(format!("https://discord.com/api/v10/channels/{}/messages/{}/reactions/{}/@me", channel_id.into(), message_id.into(), utf8_percent_encode(&emoji.into(), NON_ALPHANUMERIC)), Some(Method::PUT), None, None).await
}

pub async fn delete_message(channel_id: impl Into<String>, message_id: impl Into<String>) -> Result<()> {
	fetch_json(format!("https://discord.com/api/v10/channels/{}/messages/{}", channel_id.into(), message_id.into()), Some(Method::DELETE), None, None).await
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Guild {
	pub name: String,
	pub icon: Option<String>,
	pub splash: Option<String>,
	pub verification_level: GuildVerificationLevel
}

impl Guild {
	pub async fn fetch(guild_id: &Id<GuildMarker>) -> Result<Self> {
		Ok(match CACHES.discord_guilds.get(guild_id)
			.instrument(info_span!("cache.discord_guilds.read", ?guild_id))
			.await {
				Some(x) => x,
				None => {
					let guild: Guild = get_json(format!("https://discord.com/api/v10/guilds/{}", guild_id), None).await?;
					let span = info_span!("cache.discord_guilds.write", ?guild_id);
					CACHES.discord_guilds.insert(*guild_id, guild.clone())
						.instrument(span)
						.await;
	
					guild
				}
			}
		)
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct GuildOnboarding {
	pub enabled: bool
}

impl GuildOnboarding {
	pub async fn fetch(guild_id: &str) -> Result<Self> {
		Ok(match CACHES.discord_guild_onboarding.get(guild_id)
			.instrument(info_span!("cache.discord_guild_onboarding.read", ?guild_id))
			.await {
				Some(x) => x,
				None => {
					let onboarding: GuildOnboarding = get_json(format!("https://discord.com/api/v10/guilds/{}/onboarding", guild_id), None).await?;
					let span = info_span!("cache.discord_guild_onboarding.write", ?guild_id);
					CACHES.discord_guild_onboarding.insert(guild_id.to_string(), onboarding.clone())
						.instrument(span)
						.await;
	
					onboarding
				}
			}
		)
	}
}

#[derive(Clone, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum GuildVerificationLevel {
	None,
	Low,
	Medium,
	High,
	VeryHigh
}

pub async fn get_member(guild_id: &Id<GuildMarker>, user_id: &Id<UserMarker>) -> Result<Member> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/members/{}", guild_id, user_id), None).await
}

pub async fn get_members(guild_id: &Id<GuildMarker>) -> Result<Vec<Member>> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/members?limit=100", guild_id), None).await
}

pub async fn get_guild_roles(guild_id: &Id<GuildMarker>) -> Result<Vec<Role>> {
	get_json(format!("https://discord.com/api/v10/guilds/{}/roles", guild_id), None).await
}