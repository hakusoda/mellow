use serde::Deserialize;
use tracing::{ Instrument, info_span };

use crate::{
	cache::CACHES,
	fetch::get_json,
	database::UserConnectionOAuthAuthorisation,
	Result
};

#[derive(Clone, Debug, Deserialize)]
pub struct UserIdentity {
	pub included: Option<Vec<UserIdentityField>>
}

#[derive(Clone, Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum UserIdentityField {
	Tier,
	Member(Member),
	Campaign
}

#[derive(Clone, Debug, Deserialize)]
pub struct Member {
	pub attributes: MemberAttributes,
	pub relationships: Relationships
}

#[derive(Clone, Debug, Deserialize)]
pub struct MemberAttributes {
	pub patron_status: Option<String>
}

#[derive(Clone, Debug, Deserialize)]
pub struct Relationships {
	pub campaign: Relationship<Campaign>,
	pub currently_entitled_tiers: Relationship<CurrentlyEntitledTier>
}

#[derive(Clone, Debug, Deserialize)]
pub struct Relationship<T> {
	pub data: T
}

#[derive(Clone, Debug, Deserialize)]
pub struct Campaign {
	pub id: String
}

#[derive(Clone, Debug, Deserialize)]
pub struct CurrentlyEntitledTier(pub Vec<Tier>);

#[derive(Clone, Debug, Deserialize)]
pub struct Tier {
	pub id: String
}

pub async fn get_user_memberships(oauth_authorisation: &UserConnectionOAuthAuthorisation) -> Result<UserIdentity> {
	let access_token = &oauth_authorisation.access_token;
	Ok(match CACHES.patreon_user_identities.get(access_token)
		.instrument(info_span!("cache.patreon_user_identities.read", ?access_token))
		.await {
			Some(x) => x,
			None => {
				let mut headers = reqwest::header::HeaderMap::new();
				headers.insert("authorization", format!("{} {}", oauth_authorisation.token_type, access_token).parse().unwrap());

				let identity: UserIdentity = get_json("https://www.patreon.com/api/oauth2/v2/identity?include=memberships.campaign,memberships.currently_entitled_tiers&fields%5Bmember%5D=patron_status", Some(headers)).await?;
				
				let span = info_span!("cache.patreon_user_identities.write", ?access_token);
				CACHES.patreon_user_identities.insert(access_token.clone(), identity.clone())
					.instrument(span)
					.await;

				identity
			}
		}
	)
}