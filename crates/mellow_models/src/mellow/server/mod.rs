use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::{ DocumentMarker, SyncActionMarker, UserMarker },
		HakuId
	},
	PG_POOL
};
use serde::Deserialize;
use std::pin::Pin;
use twilight_model::id::{
	marker::{ ChannelMarker, GuildMarker },
	Id
};

use crate::{
	hakumi::OAuthAuthorisationModel,
	Result
};

pub mod command;
pub use command::CommandModel;

pub mod sync_action;
pub use sync_action::SyncActionModel;

pub mod user_settings;
pub use user_settings::UserSettingsModel;

#[derive(Debug, Deserialize)]
pub struct ServerModel {
	pub id: Id<GuildMarker>,
	pub logging_types: u8,
	pub default_nickname: Option<String>,
	pub logging_channel_id: Option<Id<ChannelMarker>>,
	pub allow_forced_syncing: bool
}

impl ServerModel {
	pub async fn get(guild_id: Id<GuildMarker>) -> Result<Option<Self>> {
		Self::get_many(&[guild_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(guild_ids: &[Id<GuildMarker>]) -> Result<Vec<Self>> {
		let guild_ids: Vec<i64> = guild_ids
			.iter()
			.map(|x| x.get() as i64)
			.collect();
		Ok(sqlx::query!(
			"
			SELECT id, logging_types, default_nickname, logging_channel_id, allow_forced_syncing
			FROM mellow_servers
			WHERE id = ANY($1)
			",
			&guild_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(Self {
					id: Id::new(record.id as u64),
					logging_types: record.logging_types as u8,
					default_nickname: record.default_nickname,
					logging_channel_id: record.logging_channel_id.map(|x| Id::new(x as u64)),
					allow_forced_syncing: record.allow_forced_syncing
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn oauth_authorisations(guild_id: Id<GuildMarker>) -> Result<Vec<OAuthAuthorisationModel>> {
		Ok(sqlx::query!(
			"
			SELECT id, expires_at, access_token, refresh_token, token_type
			FROM mellow_server_oauth_authorisations
			WHERE server_id = $1
			",
			guild_id.get() as i64
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(OAuthAuthorisationModel {
					id: record.id as u64,
					expires_at: record.expires_at,
					access_token: record.access_token,
					refresh_token: record.refresh_token,
					token_type: record.token_type
				});
				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn sync_actions(guild_id: Id<GuildMarker>) -> Result<Vec<HakuId<SyncActionMarker>>> {
		Ok(sqlx::query!(
			"
			SELECT id
			FROM mellow_server_sync_actions
			WHERE server_id = $1
			",
			guild_id.get() as i64
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(record.id.into());
				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn users(guild_id: Id<GuildMarker>) -> Result<Vec<HakuId<UserMarker>>> {
		Ok(sqlx::query!(
			"
			SELECT user_id
			FROM mellow_user_server_settings
			WHERE server_id = $1
			",
			guild_id.get() as i64
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(record.user_id.into());
				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn visual_scripting_documents(guild_id: Id<GuildMarker>) -> Result<Vec<HakuId<DocumentMarker>>> {
		Ok(sqlx::query!(
			"
			SELECT id
			FROM visual_scripting_documents
			WHERE mellow_server_id = $1
			",
			guild_id.get() as i64
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(record.id.into());
				async move { Ok(acc) }
			})
			.await?
		)
	}
}