use serde::{ Serialize, Deserialize };
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	database::DATABASE,
	database::{ ProfileSyncAction, UserConnectionOAuthAuthorisation },
	Result
};

pub mod logging;
pub mod action_log;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
	pub id: Id<GuildMarker>,
	pub actions: Vec<ProfileSyncAction>,
	pub logging_types: u8,
	pub default_nickname: Option<String>,
	pub logging_channel_id: Option<String>,
	pub oauth_authorisations: Vec<UserConnectionOAuthAuthorisation>,
	pub allow_forced_syncing: bool
}

impl Server {
	pub async fn fetch(server_id: &Id<GuildMarker>) -> Result<Self> {
		Ok(serde_json::from_str(&DATABASE.from("mellow_servers")
			.select("id,default_nickname,allow_forced_syncing,logging_types,logging_channel_id,actions:mellow_binds(id,name,type,metadata,requirements_type,requirements:mellow_bind_requirements(id,type,data)),oauth_authorisations:mellow_server_oauth_authorisations(expires_at,token_type,access_token,refresh_token)")
			.eq("id", server_id.to_string())
			.limit(1)
			.single()
			.execute()
			.await?
			.text()
			.await?
		)?)
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerSettings {
	pub user_connections: Vec<ServerSettingsUserConnection>
}

impl Default for ServerSettings {
	fn default() -> Self {
		Self {
			user_connections: vec![]
		}
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct ServerSettingsUserConnection {
	pub id: String
}