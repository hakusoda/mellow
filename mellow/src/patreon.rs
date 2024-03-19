use serde::Deserialize;

use crate::{
	fetch::get_json,
	database::UserConnectionOAuthAuthorisation,
	Result
};

#[derive(Debug, Deserialize)]
pub struct StructToBeNamed {
	pub included: Option<Vec<EnumToBeNamed>>
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum EnumToBeNamed {
	Tier,
	Member(Member),
	Campaign
}

#[derive(Debug, Deserialize)]
pub struct Member {
	pub id: String,
	pub attributes: MemberAttributes,
	pub relationships: Relationships
}

#[derive(Debug, Deserialize)]
pub struct MemberAttributes {
	pub patron_status: String
}

#[derive(Debug, Deserialize)]
pub struct Relationships {
	pub campaign: Relationship<Campaign>,
	pub currently_entitled_tiers: Relationship<CurrentlyEntitledTier>
}

#[derive(Debug, Deserialize)]
pub struct Relationship<T> {
	pub data: T
}

#[derive(Debug, Deserialize)]
pub struct Campaign {
	pub id: String
}

#[derive(Debug, Deserialize)]
pub struct CurrentlyEntitledTier(pub Vec<Tier>);

#[derive(Debug, Deserialize)]
pub struct Tier {
	pub id: String
}

pub async fn get_user_memberships(oauth_authorisation: &UserConnectionOAuthAuthorisation) -> Result<StructToBeNamed> {
	let mut headers = reqwest::header::HeaderMap::new();
	headers.insert("authorization", format!("{} {}", oauth_authorisation.token_type, oauth_authorisation.access_token).parse().unwrap());

	get_json("https://www.patreon.com/api/oauth2/v2/identity?include=memberships.campaign,memberships.currently_entitled_tiers&fields%5Bmember%5D=patron_status", Some(headers)).await
}