use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct UserIdentityModel {
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