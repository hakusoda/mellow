use serde::Deserialize;
use chrono::{ Utc, TimeDelta };
use reqwest::Method;
use dashmap::mapref::one::Ref;
use url_escape::encode_component;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	fetch::fetch_json,
	model::{
		mellow::{
			server::UserSettings,
			MELLOW_MODELS
		},
		hakumi::id::{
			marker::UserMarker,
			HakuId
		}
	},
	database::DATABASE,
	Result
};
use connection::ConnectionKind;

pub mod connection;
pub use connection::Connection;

#[derive(Clone, Debug, Deserialize)]
pub struct User {
	pub id: HakuId<UserMarker>,
	pub connections: Vec<Connection>
}

impl User {
	pub async fn server_settings(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, (Id<GuildMarker>, HakuId<UserMarker>), UserSettings>> {
		MELLOW_MODELS.member_settings(guild_id, self.id).await
	}

	pub async fn server_connections(&self, guild_id: Id<GuildMarker>) -> Result<Vec<&Connection>> {
		let settings = self.server_settings(guild_id).await?;
		Ok(settings.server_connections(self))
	}
	
	pub fn has_connection(&self, sub: &str, connection_kind: ConnectionKind) -> bool {
		self.connections.iter().any(|x| x.kind == connection_kind && x.sub == sub)
	}

	pub async fn refresh_oauth(&mut self) -> Result<()> {
		for connection in self.connections.iter_mut() {
			for oauth_authorisation in connection.oauth_authorisations.iter_mut() {
				if Utc::now() > oauth_authorisation.expires_at {
					let mut url = reqwest::Url::parse("https://www.patreon.com/api/oauth2/token")?;
					url.set_query(Some(&format!("client_id={}&client_secret={}&grant_type=refresh_token&refresh_token={}",
						encode_component(env!("PATREON_CLIENT_ID")),
						encode_component(env!("PATREON_CLIENT_SECRET")),
						encode_component(&oauth_authorisation.refresh_token)
					)));
			
					let result: PatreonRefreshResult = fetch_json(url, Some(Method::POST), None, None).await?;
					let expires_at = Utc::now().checked_add_signed(TimeDelta::seconds(result.expires_in)).unwrap();
					DATABASE
						.from("user_connection_oauth_authorisations")
						.update(simd_json::json!({
							"expires_at": expires_at,
							"token_type": result.token_type,
							"access_token": result.access_token,
							"refresh_token": result.refresh_token
						}))?
						.eq("id", oauth_authorisation.id)
						.header("x-skip-model-update", "warwithoutreason")
						.await?;

					oauth_authorisation.expires_at = expires_at;
					oauth_authorisation.token_type = result.token_type;
					oauth_authorisation.access_token = result.access_token;
					oauth_authorisation.refresh_token = result.refresh_token;
				}
			}
		}
		Ok(())
	}
}

#[derive(Deserialize)]
pub struct PatreonRefreshResult {
	expires_in: i64,
	token_type: String,
	access_token: String,
	refresh_token: String
}