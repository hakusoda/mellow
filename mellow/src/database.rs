use serde::{ Serialize, Deserialize };
use once_cell::sync::Lazy;
use postgrest::Postgrest;
use serde_repr::*;

pub const DATABASE: Lazy<Postgrest> = Lazy::new(|| {
	let key = env!("SUPABASE_API_KEY");
	Postgrest::new("https://hakumi.supabase.co/rest/v1")
		.insert_header("apikey", key)
		.insert_header("authorization", format!("Bearer {}", key))
});

#[derive(Deserialize, Clone, Debug)]
pub struct User {
	pub id: String,
	pub connections: Vec<UserServerConnection>
}

#[derive(Serialize_repr, Deserialize_repr, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum UserConnectionKind {
	Discord,
	GitHub,
	Roblox
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserConnection {
	pub sub: String,
	#[serde(rename = "type")]
	pub kind: UserConnectionKind,
	pub username: Option<String>,
	pub display_name: Option<String>
}

impl UserConnection {
	pub fn display(&self) -> String {
		let sub = &self.sub;
		let name = self.display_name.clone().unwrap_or("Unknown".into());
		let username = self.username.clone().unwrap_or("@unknown".into());
		match self.kind {
			UserConnectionKind::Discord => format!("<:discord:1137058089980416080> Discord — [{name}](https://discord.com/users/{sub})"),
			UserConnectionKind::GitHub => format!("<:github:1143983126792642661> GitHub — [{name}](https://github.com/{username})"),
			UserConnectionKind::Roblox => format!("<:roblox:1175038688271536169> Roblox — [{name}](https://www.roblox.com/users/{sub})")
		}
	}
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct UserServerConnection {
	pub id: String,
	pub connection: UserConnection
}

#[derive(Deserialize, Clone, Debug)]
pub struct UserResponse {
	pub sub: String,
	pub user: User
}

pub async fn get_users_by_discord(ids: Vec<String>, server_id: String) -> Vec<UserResponse> {
	serde_json::from_str(&DATABASE.from("user_connections")
		.select("sub,user:users(id,connections:mellow_user_server_connections(id,connection:user_connections(sub,type,username,display_name)))")
		.in_("sub", ids)
		.eq("users.mellow_user_server_connections.server_id", server_id)
		.execute().await.unwrap().text().await.unwrap()
	).unwrap()
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Server {
	pub id: String,
	pub actions: Vec<ProfileSyncAction>,
	pub logging_types: u8,
	pub default_nickname: Option<String>,
	pub logging_channel_id: Option<String>,
	pub allow_forced_syncing: bool
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileSyncAction {
	pub id: String,
	pub name: String,
	#[serde(rename = "type")]
	pub kind: ProfileSyncActionKind,
	pub metadata: serde_json::Value,
	pub requirements: Vec<ProfileSyncActionRequirement>,
	pub requirements_type: ProfileSyncActionRequirementsKind
}

#[derive(Deserialize, Debug, Clone, Eq, Hash, PartialEq, Serialize)]
pub struct ProfileSyncActionRequirement {
	pub id: String,
	#[serde(rename = "type")]
	pub kind: ProfileSyncActionRequirementKind,
	pub data: Vec<String>
}

impl ProfileSyncActionRequirement {
	pub fn relevant_connection(&self) -> Option<UserConnectionKind> {
		match self.kind {
			ProfileSyncActionRequirementKind::RobloxHaveConnection |
			ProfileSyncActionRequirementKind::RobloxHaveGroupRole |
			ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange |
			ProfileSyncActionRequirementKind::RobloxInGroup |
			ProfileSyncActionRequirementKind::RobloxBeFriendsWith |
			ProfileSyncActionRequirementKind::RobloxHaveAsset |
			ProfileSyncActionRequirementKind::RobloxHaveBadge |
			ProfileSyncActionRequirementKind::RobloxHavePass => Some(UserConnectionKind::Roblox),
			ProfileSyncActionRequirementKind::GitHubInOrganisation => Some(UserConnectionKind::GitHub),
			_ => None
		}
	}
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ProfileSyncActionKind {
	GiveRoles,
	BanFromServer,
	KickFromServer,
	CancelSync
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ProfileSyncActionRequirementKind {
	RobloxHaveConnection,
	RobloxHaveGroupRole,
	RobloxHaveGroupRankInRange,
	RobloxInGroup,
	RobloxBeFriendsWith,
	MeetOtherAction,
	HAKUMIInTeam,
	SteamInGroup,
	RobloxHaveAsset,
	RobloxHaveBadge,
	RobloxHavePass,
	GitHubInOrganisation
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ProfileSyncActionRequirementsKind {
	MeetAll,
	MeetOne
}

pub async fn get_server(id: impl Into<String>) -> Server {
	serde_json::from_str(&DATABASE.from("mellow_servers")
		.select("id,default_nickname,allow_forced_syncing,logging_types,logging_channel_id,actions:mellow_binds(id,name,type,metadata,requirements_type,requirements:mellow_bind_requirements(id,type,data))")
		.eq("id", id.into())
		.limit(1)
		.single()
		.execute()
		.await
		.unwrap()
		.text()
		.await
		.unwrap()
	).unwrap()
}

pub async fn server_exists(id: impl Into<String>) -> bool {
	// this isn't an ideal method, but this rust library is way too limited, especially when compared to postgrest-js...
	DATABASE.from("mellow_servers")
		.select("id")
		.eq("id", id.into())
		.limit(1)
		.single()
		.execute()
		.await
		.unwrap()
		.status()
		.is_success()
}