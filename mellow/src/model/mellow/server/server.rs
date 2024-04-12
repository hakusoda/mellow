use serde::Deserialize;
use twilight_model::id::{
	marker::{ GuildMarker, ChannelMarker },
	Id
};

use super::SyncAction;
use crate::{
	model::hakumi::user::{ connection::ConnectionOAuthAuthorisation, User },
	database::DATABASE,
	Result
};

#[derive(Debug, Deserialize)]
pub struct Server {
	pub id: Id<GuildMarker>,
	pub actions: Vec<SyncAction>,
	pub logging_types: u8,
	pub default_nickname: Option<String>,
	pub logging_channel_id: Option<Id<ChannelMarker>>,
	pub allow_forced_syncing: bool,
	pub oauth_authorisations: Vec<ConnectionOAuthAuthorisation>
}

impl Server {
	pub async fn users(&self, guild_id: Id<GuildMarker>) -> Result<Vec<User>> {
		Ok(simd_json::from_slice(&mut DATABASE.from("mellow_user_server_settings")
			.select("...users(id,server_settings:mellow_user_server_settings(user_connections),connections:user_connections(id,sub,type,username,display_name,oauth_authorisations:user_connection_oauth_authorisations(token_type,expires_at,access_token,refresh_token)))")
			.eq("server_id", guild_id.to_string())
			.eq("users.mellow_user_server_settings.server_id", guild_id.to_string())
			.execute()
			.await?
			.bytes()
			.await?
			.to_vec()
		)?)
	}
}