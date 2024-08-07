use chrono::{ TimeDelta, Utc };
use dashmap::{
	mapref::one::Ref,
	DashMap
};
use mellow_models::patreon::{
	campaign::{ GetCampaign, Tier },
	CampaignModel, UserIdentityModel
};
use mellow_util::{
	hakuid::{
		marker::ConnectionMarker,
		HakuId
	},
	HTTP,
	PG_POOL,
	get_json
};
use reqwest::StatusCode;
use serde::Deserialize;
use std::{
	collections::HashMap,
	pin::Pin
};
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{ CACHE, Error, Result };

const PATREON_CLIENT_ID: &str = env!("PATREON_CLIENT_ID");
const PATREON_CLIENT_SECRET: &str = env!("PATREON_CLIENT_SECRET");

#[derive(Deserialize)]
pub struct PatreonRefreshResult {
	expires_in: i64,
	token_type: String,
	access_token: String,
	refresh_token: String
}

#[derive(Default)]
pub struct PatreonCache {
	campaigns: DashMap<Id<GuildMarker>, CampaignModel>,
	user_identities: DashMap<HakuId<ConnectionMarker>, UserIdentityModel>
}

impl PatreonCache {
	pub async fn campaign(&self, guild_id: Id<GuildMarker>) -> Result<Option<Ref<'_, Id<GuildMarker>, CampaignModel>>> {
		Ok(match self.campaigns.get(&guild_id) {
			Some(model) => Some(model),
			None => if
				let Some(authorisation_id) = CACHE
					.mellow
					.server_oauth_authorisations(guild_id)
					.await?
					.into_iter()
					.next() &&
				let Some(authorisation) = CACHE.mellow.oauth_authorisation(authorisation_id)
			{
				let auth_header = if Utc::now() > authorisation.expires_at {
					let response = HTTP
						.post("https://www.patreon.com/api/oauth2/token")
						.form(&HashMap::from([
							("client_id", PATREON_CLIENT_ID),
							("client_secret", PATREON_CLIENT_SECRET),
							("grant_type", "refresh_token"),
							("refresh_token", &authorisation.refresh_token)
						]))
						.send()
						.await?;
					let status_code = response.status();
					if status_code.is_success() {
						let result: PatreonRefreshResult = response
							.json()
							.await?;
						let expires_at = Utc::now().checked_add_signed(TimeDelta::seconds(result.expires_in)).unwrap();
						sqlx::query!(
							"
							UPDATE user_connection_oauth_authorisations
							SET expires_at = $2, token_type = $3, access_token = $4, refresh_token = $5
							WHERE id = $1
							",
							authorisation.id as i64,
							expires_at,
							result.token_type,
							result.access_token,
							result.refresh_token
						)
							.execute(&*Pin::static_ref(&PG_POOL).await)
							.await?;

						let auth_header = format!("{} {}", result.token_type, result.access_token);
						if let Some(mut authorisation) = CACHE.mellow.oauth_authorisations.get_mut(&authorisation_id) {
							authorisation.expires_at = expires_at;
							authorisation.token_type = result.token_type;
							authorisation.access_token = result.access_token;
							authorisation.refresh_token = result.refresh_token;
						}

						auth_header
					} else if status_code == StatusCode::UNAUTHORIZED {
						return Ok(None);
					} else {
						return Err(Error::OAuthAuthorisationRefresh);
					}
				} else { format!("{} {}", authorisation.token_type, authorisation.access_token) };

				let campaign: GetCampaign = get_json("https://www.patreon.com/api/oauth2/v2/campaigns?include=tiers&fields%5Btier%5D=patron_count")
					.header("authorization", auth_header)
					.await?;
				let Some(included) = campaign.included else {
					return Err(Error::ModelNotFound);
				};
				Some(self.campaigns.entry(guild_id)
					.insert(CampaignModel {
						tiers: included
							.into_iter()
							.map(|x| Tier {
								patron_count: x.attributes.patron_count
							})
							.collect()
					})
					.downgrade()
				)
			} else { None }
		})
	}

	pub async fn user_identity(&self, connection_id: HakuId<ConnectionMarker>) -> Result<Option<Ref<'_, HakuId<ConnectionMarker>, UserIdentityModel>>> {
		Ok(match self.user_identities.get(&connection_id) {
			Some(model) => Some(model),
			None => {
				let connection = CACHE
					.hakumi
					.connection(connection_id)
					.await?;
				if let Some(mut authorisation) = connection.oauth_authorisations.first().cloned() {
					drop(connection);

					if Utc::now() > authorisation.expires_at {
						let response = HTTP
							.post("https://www.patreon.com/api/oauth2/token")
							.form(&HashMap::from([
								("client_id", PATREON_CLIENT_ID),
								("client_secret", PATREON_CLIENT_SECRET),
								("grant_type", "refresh_token"),
								("refresh_token", &authorisation.refresh_token)
							]))
							.send()
							.await?;
						let status_code = response.status();
						if status_code.is_success() {
							let result: PatreonRefreshResult = response
								.json()
								.await?;
							let expires_at = Utc::now().checked_add_signed(TimeDelta::seconds(result.expires_in)).unwrap();
							sqlx::query!(
								"
								UPDATE user_connection_oauth_authorisations
								SET expires_at = $2, token_type = $3, access_token = $4, refresh_token = $5
								WHERE id = $1
								",
								authorisation.id as i64,
								expires_at,
								result.token_type,
								result.access_token,
								result.refresh_token
							)
								.execute(&*Pin::static_ref(&PG_POOL).await)
								.await?;

							if
								let Some(mut connection) = CACHE.hakumi.connections.get_mut(&connection_id) &&
								let Some(authorisation2) = connection.oauth_authorisations.first_mut()
							{
								authorisation2.expires_at = expires_at;
								authorisation2.token_type = result.token_type;
								authorisation2.access_token = result.access_token;
								authorisation2.refresh_token = result.refresh_token;

								authorisation = authorisation2.clone();
							}
						} else if status_code == StatusCode::UNAUTHORIZED {
							return Ok(None);
						} else {
							return Err(Error::OAuthAuthorisationRefresh);
						}
					}

					let access_token = authorisation.access_token.clone();
					let token_type = authorisation.token_type.clone();
					let new_model: UserIdentityModel = get_json("https://www.patreon.com/api/oauth2/v2/identity?include=memberships.campaign,memberships.currently_entitled_tiers&fields%5Bmember%5D=patron_status")
						.header("authorization", format!("{token_type} {access_token}"))
						.await?;
					Some(self.user_identities.entry(connection_id)
						.insert(new_model)
						.downgrade()
					)
				} else { None }
			}
		})
	}
}