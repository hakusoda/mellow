use serde::Deserialize;
use tracing::{ Instrument, info_span };

use crate::{
	cache::CACHES,
	fetch::get_json,
	model::hakumi::user::connection::OAuthAuthorisation,
	visual_scripting::{ Variable, VariableKind },
	Result
};

#[derive(Clone, Debug, Deserialize)]
pub struct UserIdentity {
	#[serde(default)]
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

pub async fn get_user_memberships(oauth_authorisation: &OAuthAuthorisation) -> Result<UserIdentity> {
	let access_token = oauth_authorisation.access_token.clone();
	Ok(match CACHES.patreon_user_identities.get(&access_token)
		.instrument(info_span!("cache.patreon_user_identities.read", ?access_token))
		.await {
			Some(x) => x,
			None => {
				let mut headers = reqwest::header::HeaderMap::new();
				headers.insert("authorization", format!("{} {}", oauth_authorisation.token_type, access_token).parse().unwrap());
				headers.insert("content-type", "application/json".parse().unwrap());

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

#[derive(Clone)]
pub struct Campaign2 {
	pub tiers: Vec<Tier2>
}

impl From<Campaign2> for Variable {
	fn from(value: Campaign2) -> Self {
		Variable::create_map([
			("tiers", VariableKind::List(value.tiers.into_iter().map(|x| Variable::create_map([
				("patron_count", x.patron_count.into())
			], None)).collect()).into())
		], None)
	}
}

#[derive(Clone)]
pub struct Tier2 {
	pub patron_count: u64
}

#[derive(Deserialize)]
struct GetCampaign {
	included: Vec<IncludedItem>
}

#[derive(Deserialize)]
struct IncludedItem {
	attributes: IncludedItemAttributes
}

#[derive(Deserialize)]
struct IncludedItemAttributes {
	patron_count: u64
}

pub async fn get_campaign(oauth_authorisation: OAuthAuthorisation) -> Result<Campaign2> {
	let access_token = &oauth_authorisation.access_token;
	Ok(match CACHES.patreon_campaigns.get(access_token)
		.instrument(info_span!("cache.patreon_campaigns.read", ?access_token))
		.await {
			Some(x) => x,
			None => {
				let mut headers = reqwest::header::HeaderMap::new();
				headers.insert("authorization", format!("{} {}", oauth_authorisation.token_type, access_token).parse().unwrap());

				let campaign: GetCampaign = get_json("https://www.patreon.com/api/oauth2/v2/campaigns?include=tiers&fields%5Btier%5D=patron_count", Some(headers)).await?;
				
				let span = info_span!("cache.patreon_campaigns.write", ?access_token);
				let mapped = Campaign2 {
					tiers: campaign.included.into_iter().map(|x| Tier2 {
						patron_count: x.attributes.patron_count
					}).collect()
				};
				CACHES.patreon_campaigns.insert(access_token.clone(), mapped.clone())
					.instrument(span)
					.await;

				mapped
			}
		}
	)
}