use dashmap::DashMap;
use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::{ ConnectionMarker, UserMarker },
		HakuId
	},
	PG_POOL
};
use std::pin::Pin;

use crate::Result;

pub mod connection;
pub use connection::ConnectionModel;

#[derive(Clone, Debug)]
pub struct UserModel {
	pub id: HakuId<UserMarker>,
	pub name: Option<String>,
	pub username: String,
	pub avatar_url: Option<String>
}

impl UserModel {
	pub fn display_name(&self) -> &str {
		self.name
			.as_ref()
			.map_or_else(|| &self.username, |x| x)
	}

	pub async fn get(user_id: HakuId<UserMarker>) -> Result<Option<Self>> {
		Self::get_many(&[user_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(user_ids: &[HakuId<UserMarker>]) -> Result<Vec<Self>> {
		if user_ids.is_empty() {
			return Ok(vec![]);
		}

		let user_ids: Vec<_> = user_ids
			.iter()
			.map(|x| x.value)
			.collect();
		Ok(sqlx::query!(
			r#"
			SELECT id, name, username, avatar_url
			FROM users
			WHERE id = ANY($1)
			"#,
			&user_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(Self {
					id: record.id.into(),
					name: record.name,
					username: record.username,
					avatar_url: record.avatar_url
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn connections_many(user_ids: &[HakuId<UserMarker>]) -> Result<DashMap<HakuId<UserMarker>, Vec<HakuId<ConnectionMarker>>>> {
		if user_ids.is_empty() {
			return Ok(DashMap::new());
		}

		let user_ids: Vec<_> = user_ids
			.iter()
			.map(|x| x.value)
			.collect();
		Ok(sqlx::query!(
			r#"
			SELECT id, user_id
			FROM user_connections
			WHERE user_id = ANY($1)
			"#,
			&user_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(DashMap::<HakuId<UserMarker>, Vec<HakuId<ConnectionMarker>>>::new(), |acc, record| {
				acc
					.entry(record.user_id.into())
					.or_default()
					.push(record.id.into());

				async move { Ok(acc) }
			})
			.await?
		)
	}
}

/*impl UserModel {
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
}*/

pub struct PatreonRefreshResult {
	/*expires_in: i64,
	token_type: String,
	access_token: String,
	refresh_token: String*/
}