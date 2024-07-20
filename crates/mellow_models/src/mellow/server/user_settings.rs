use mellow_util::{
	hakuid::{
		marker::{ ConnectionMarker, UserMarker },
		HakuId
	},
	PG_POOL
};
use serde::Deserialize;
use std::pin::Pin;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::Result;

#[derive(Clone, Default)]
pub struct UserSettingsModel {
	pub user_connections: Vec<ConnectionReference>
}

impl UserSettingsModel {
	pub async fn get(guild_id: Id<GuildMarker>, user_id: HakuId<UserMarker>) -> Result<Self> {
		Ok(if let Some(record) = sqlx::query!(
			"
			SELECT user_connections
			FROM mellow_user_server_settings
			WHERE server_id = $1 AND user_id = $2
			",
			guild_id.get() as i64,
			user_id.value
		)
			.fetch_optional(&*Pin::static_ref(&PG_POOL).await)
			.await?
		{
			Self {
				user_connections: serde_json::from_value(record.user_connections)?
			}
		} else { Self::default() })
	}

	pub fn user_connections(&self) -> Vec<HakuId<ConnectionMarker>> {
		self.user_connections
			.iter()
			.map(|x| x.id)
			.collect()
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionReference {
	pub id: HakuId<ConnectionMarker>
}