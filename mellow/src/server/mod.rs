use serde::{ Serialize, Deserialize };

use crate::{
	database::DATABASE,
	database::ProfileSyncAction,
	Result
};

pub mod logging;
pub mod action_log;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
	pub id: String,
	pub actions: Vec<ProfileSyncAction>,
	pub logging_types: u8,
	pub default_nickname: Option<String>,
	pub logging_channel_id: Option<String>,
	pub allow_forced_syncing: bool
}

impl Server {
	pub async fn fetch(server_id: impl Into<String>) -> Result<Self> {
		Ok(serde_json::from_str(&DATABASE.from("mellow_servers")
			.select("id,default_nickname,allow_forced_syncing,logging_types,logging_channel_id,actions:mellow_binds(id,name,type,metadata,requirements_type,requirements:mellow_bind_requirements(id,type,data))")
			.eq("id", server_id.into())
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