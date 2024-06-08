use serde::Deserialize;
use futures::TryStreamExt;
use twilight_model::id::{
	marker::{ GuildMarker, ChannelMarker },
	Id
};

use crate::{
	util::deserialise_nullable_vec,
	state::STATE,
	model::hakumi::{
		user::{ connection::OAuthAuthorisation, User },
		HakuId
	},
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
	pub async fn get(guild_id: Id<GuildMarker>) -> Result<Self> {
		let query_id = guild_id.get() as i64;
		let actions: Vec<SyncAction> = sqlx::query!(
			"
			SELECT id, kind, criteria, action_data, display_name
			FROM mellow_server_sync_actions
			WHERE server_id = $1
			",
			query_id
		)
			.fetch(&STATE.get().unwrap().pg_pool)
			.try_fold(Vec::new(), |mut acc, m| {
				acc.push(SyncAction {
					id: HakuId::new(m.id),
					kind: serde_json::from_value(serde_json::json!({
						"kind": m.kind,
						"action_data": m.action_data
					})).unwrap(),
					criteria: serde_json::from_value(m.criteria).unwrap(),
					display_name: m.display_name
				});
				async move { Ok(acc) }
			})
			.await?;

		let oauth_authorisations: Vec<OAuthAuthorisation> = sqlx::query!(
			"
			SELECT id, token_type, expires_at, access_token, refresh_token
			FROM mellow_server_oauth_authorisations
			WHERE server_id = $1
			",
			query_id
		)
			.fetch(&STATE.get().unwrap().pg_pool)
			.try_fold(Vec::new(), |mut acc, m| {
				acc.push(OAuthAuthorisation {
					id: m.id as u64,
					token_type: m.token_type,
					expires_at: m.expires_at,
					access_token: m.access_token,
					refresh_token: m.refresh_token
				});
				async move { Ok(acc) }
			})
			.await?;

		let server_record = sqlx::query!(
			"
			SELECT id, logging_types, default_nickname, logging_channel_id, allow_forced_syncing
			FROM mellow_servers
			WHERE id = $1
			",
			query_id
		)
			.fetch_one(&STATE.get().unwrap().pg_pool)
			.await?;
		
		Ok(Self {
			id: Id::new(server_record.id as u64),
			actions,
			logging_types: server_record.logging_types as u8,
			default_nickname: server_record.default_nickname,
			logging_channel_id: server_record.logging_channel_id.map(|x| Id::new(x.parse().unwrap())),
			oauth_authorisations,
			allow_forced_syncing: server_record.allow_forced_syncing
		})
	}

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