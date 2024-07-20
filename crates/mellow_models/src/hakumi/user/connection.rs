use dashmap::DashMap;
use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::{ ConnectionMarker, UserMarker as HakuUserMarker },
		HakuId
	},
	PG_POOL
};
use num_derive::FromPrimitive;
use num_traits::FromPrimitive;
use serde::Deserialize;
use serde_repr::Deserialize_repr;
use std::pin::Pin;
use twilight_model::id::{
	marker::UserMarker,
	Id
};

use crate::{
	hakumi::OAuthAuthorisationModel,
	Result
};

#[derive(Debug, Deserialize)]
pub struct ConnectionModel {
	pub id: HakuId<ConnectionMarker>,
	pub sub: String,
	pub kind: ConnectionKind,
	pub username: Option<String>,
	pub display_name: Option<String>,
	pub oauth_authorisations: Vec<OAuthAuthorisationModel>,
	pub user_id: HakuId<HakuUserMarker>
}

impl ConnectionModel {
	pub async fn get(connection_id: HakuId<ConnectionMarker>) -> Result<Option<Self>> {
		Self::get_many(&[connection_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(connection_ids: &[HakuId<ConnectionMarker>]) -> Result<Vec<Self>> {
		let connection_ids: Vec<_> = connection_ids
			.iter()
			.map(|x| x.value)
			.collect();

		let mut transaction = Pin::static_ref(&PG_POOL)
			.await
			.begin()
			.await?;
		let oauth_authorisations = sqlx::query!(
			"
			SELECT id, connection_id, token_type, expires_at, access_token, refresh_token
			FROM user_connection_oauth_authorisations
			WHERE connection_id = ANY($1)
			",
			&connection_ids
		)
			.fetch(&mut *transaction)
			.try_fold(DashMap::<HakuId<ConnectionMarker>, Vec<OAuthAuthorisationModel>>::new(), |acc, record| {
				acc.entry(record.connection_id.into())
					.or_default()
					.push(OAuthAuthorisationModel {
						id: record.id as u64,
						token_type: record.token_type,
						expires_at: record.expires_at,
						access_token: record.access_token,
						refresh_token: record.refresh_token
					});
				async move { Ok(acc) }
			})
			.await?;

		let connections = sqlx::query!(
			"
			SELECT id, sub, type as kind, username, display_name, user_id
			FROM user_connections
			WHERE id = ANY($1)
			",
			&connection_ids
		)
			.fetch(&mut *transaction)
			.try_fold(Vec::new(), |mut acc, record| {
				let id: HakuId<ConnectionMarker> = record.id.into();
				acc.push(Self {
					id,
					sub: record.sub,
					kind: ConnectionKind::from_i16(record.kind).unwrap(),
					username: record.username,
					display_name: record.display_name,
					oauth_authorisations: oauth_authorisations
						.remove(&id)
						.map(|x| x.1)
						.unwrap_or_default(),
					user_id: record.user_id.into()
				});
				async move { Ok(acc) }
			})
			.await?;

		Ok(connections)
	}

	pub async fn user_discord(user_id: Id<UserMarker>) -> Result<Option<HakuId<HakuUserMarker>>> {
		Ok(sqlx::query!(
			"
			SELECT user_id
			FROM user_connections
			WHERE sub = $1
			",
			user_id.to_string()
		)
			.fetch_optional(&*Pin::static_ref(&PG_POOL).await)
			.await?
			.map(|x| x.user_id.into())
		)
	}

	pub fn display(&self) -> String {
		let sub = &self.sub;
		let name = self.display_name.clone().unwrap_or("Unknown".into());
		let username = self.username.clone().unwrap_or("@unknown".into());
		match self.kind {
			ConnectionKind::Discord => format!("<:discord:1137058089980416080> Discord — [{name}](https://discord.com/users/{sub})"),
			ConnectionKind::GitHub => format!("<:github:1143983126792642661> GitHub — [{name}](https://github.com/{username})"),
			ConnectionKind::Roblox => format!("<:roblox:1175038688271536169> Roblox — [{name}](https://www.roblox.com/users/{sub})"),
			ConnectionKind::YouTube => "placeholder".into(),
			ConnectionKind::Patreon => format!("<:Patreon:1219706758742933586> Patreon — [{name}](https://www.patreon.com/user?u={sub})"),
		}
	}

	pub fn is_discord(&self) -> bool {
		matches!(self.kind, ConnectionKind::Discord)
	}

	pub fn is_patreon(&self) -> bool {
		matches!(self.kind, ConnectionKind::Patreon)
	}

	pub fn is_roblox(&self) -> bool {
		matches!(self.kind, ConnectionKind::Roblox)
	}
}

#[derive(Clone, Debug, Deserialize_repr, FromPrimitive, PartialEq)]
#[repr(u8)]
pub enum ConnectionKind {
	Discord,
	GitHub,
	Roblox,
	YouTube,
	Patreon
}