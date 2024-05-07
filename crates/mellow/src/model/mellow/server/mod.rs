use serde::Deserialize;
use twilight_model::id::{
	marker::{ GuildMarker, ChannelMarker },
	Id
};

use crate::{
	util::deserialise_nullable_vec,
	model::hakumi::user::{ connection::OAuthAuthorisation, User },
	database::DATABASE,
	Result
};

pub mod sync_action;
pub use sync_action::SyncAction;

pub mod user_settings;
pub use user_settings::UserSettings;

#[derive(Debug, Deserialize)]
pub struct Server {
	pub id: Id<GuildMarker>,
	#[serde(default, deserialize_with = "deserialise_nullable_vec")]
	pub actions: Vec<SyncAction>,
	pub logging_types: u8,
	#[serde(default)]
	pub default_nickname: Option<String>,
	#[serde(default)]
	pub logging_channel_id: Option<Id<ChannelMarker>>,
	pub allow_forced_syncing: bool,
	#[serde(default, deserialize_with = "deserialise_nullable_vec")]
	pub oauth_authorisations: Vec<OAuthAuthorisation>
}

impl Server {
	pub async fn users(&self, guild_id: Id<GuildMarker>) -> Result<Vec<User>> {
		Ok(DATABASE.from("mellow_user_server_settings")
			.select("...users(id,server_settings:mellow_user_server_settings(user_connections),connections:user_connections(id,sub,type,username,display_name,oauth_authorisations:user_connection_oauth_authorisations(id,token_type,expires_at,access_token,refresh_token)))")
			.eq("server_id", guild_id.to_string())
			.eq("users.mellow_user_server_settings.server_id", guild_id.to_string())
			.await?
			.value
		)
	}
}