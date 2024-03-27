use serde::Deserialize;
use reqwest::{ header, Client };
use once_cell::sync::Lazy;

use crate::Result;

const CLIENT: Lazy<Client> = Lazy::new(||
	Client::builder()
		.default_headers({
			let mut headers = header::HeaderMap::new();
			headers.append("x-api-key", env!("ROBLOX_OPEN_CLOUD_KEY").parse().unwrap());
			headers
		})
		.build()
		.unwrap()
);

/*#[derive(Deserialize, Debug)]
pub struct GroupMembership {
	pub path: String,
	pub role: String,
	pub user: String,
	pub create_time: String,
	pub update_time: String
}

#[derive(Deserialize, Debug)]
struct GroupMembershipsResponse {
	group_memberships: Vec<GroupMembership>
}

pub async fn get_group_memberships(group_id: impl Into<String>, filter: Option<String>) -> Vec<GroupMembership> {
	CLIENT.get(format!("https://apis.roblox.com/cloud/v2/groups/{}/memberships?maxPageSize=100", group_id.into()))
		.query(&["filter", &filter.unwrap_or("".into())])
		.send()
		.await
		.unwrap()
		.json::<GroupMembershipsResponse>()
		.await
		.unwrap()
		.group_memberships
}*/

#[derive(Deserialize, Debug)]
pub struct UserGroupRole {
	pub role: PartialGroupRole,
	pub group: PartialGroup
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct PartialGroup {
	pub id: u128
}

#[derive(Deserialize, Debug)]
pub struct PartialGroupRole {
	pub id: u128,
	pub rank: u8
}

#[derive(Deserialize, Debug)]
pub struct UserGroupRolesResponse {
	data: Vec<UserGroupRole>
}

pub async fn get_user_group_roles(user_id: impl Into<String>) -> Result<Vec<UserGroupRole>> {
	Ok(CLIENT.get(format!("https://groups.roblox.com/v2/users/{}/groups/roles", user_id.into()))
		.send()
		.await?
		.json::<UserGroupRolesResponse>()
		.await?
		.data
	)
}