use chrono::{ TimeDelta, Utc };
use dashmap::{
	mapref::one::Ref,
	DashMap
};
use mellow_models::{
	hakumi::{
		user::connection::ConnectionKind,
		OAuthAuthorisationModel
	},
	patreon::{
		campaign::{ GetCampaign, Tier },
		CampaignModel, UserIdentityModel
	}
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
	campaigns: DashMap<String, CampaignModel>,
	user_identities: DashMap<HakuId<ConnectionMarker>, UserIdentityModel>
}

impl PatreonCache {
	pub async fn campaign(&self, oauth_authorisation: OAuthAuthorisationModel) -> Result<Ref<'_, String, CampaignModel>> {
		let access_token = oauth_authorisation.access_token;
		Ok(match self.campaigns.get(&access_token) {
			Some(model) => model,
			None => {
				let campaign: GetCampaign = get_json("https://www.patreon.com/api/oauth2/v2/campaigns?include=tiers&fields%5Btier%5D=patron_count")
					.header("authorization", format!("{} {}", oauth_authorisation.token_type, access_token))
					.await?;
				let Some(included) = campaign.included else {
					return Err(Error::ModelNotFound);
				};
				self.campaigns.entry(access_token)
					.insert(CampaignModel {
						tiers: included
							.into_iter()
							.map(|x| Tier {
								patron_count: x.attributes.patron_count
							})
							.collect()
					})
					.downgrade()
			}
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
							return Err(Error::UserConnectionInvalid(ConnectionKind::Patreon));
						} else {
							return Err(Error::UserConnectionRefresh);
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