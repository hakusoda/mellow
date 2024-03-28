use serde::{ de::Deserializer, Serialize, Deserialize };
use tracing::{ Instrument, info_span };
use once_cell::sync::Lazy;
use postgrest::Postgrest;
use serde_repr::*;
use crate::{
	cache::CACHES,
	server::ServerSettings,
	visual_scripting::{ Document, DocumentKind },
	Result
};

pub const DATABASE: Lazy<Postgrest> = Lazy::new(|| {
	let key = env!("SUPABASE_API_KEY");
	Postgrest::new("https://hakumi.supabase.co/rest/v1")
		.insert_header("apikey", key)
		.insert_header("authorization", format!("Bearer {}", key))
});

#[derive(Deserialize, Clone, Debug)]
pub struct User {
	pub id: String,
	pub connections: Vec<UserConnection>,
	#[serde(deserialize_with = "deserialise_user_server_settings")]
	pub server_settings: [ServerSettings; 1]
}

fn deserialise_user_server_settings<'de, D: Deserializer<'de>>(deserialiser: D) -> core::result::Result<[ServerSettings; 1], D::Error> {
	Vec::<ServerSettings>::deserialize(deserialiser)
		.map(|x| match x.is_empty() {
			true => [ServerSettings::default()],
			false => [x[0].clone()]
		})
}

impl User {
	pub fn server_settings(&self) -> &ServerSettings {
		&self.server_settings[0]
	}
	pub fn server_connections(&self) -> Vec<&UserConnection> {
		let server_connections = &self.server_settings().user_connections;
		self.connections.iter().filter(|x| server_connections.iter().find(|y| y.id == x.id).is_some()).collect()
	}
}

#[derive(Serialize_repr, Deserialize_repr, Clone, Debug, PartialEq)]
#[repr(u8)]
pub enum UserConnectionKind {
	Discord,
	GitHub,
	Roblox,
	YouTube,
	Patreon
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserConnection {
	pub id: String,
	pub sub: String,
	#[serde(rename = "type")]
	pub kind: UserConnectionKind,
	pub username: Option<String>,
	pub display_name: Option<String>,
	pub oauth_authorisations: Option<Vec<UserConnectionOAuthAuthorisation>>
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct UserConnectionOAuthAuthorisation {
	pub token_type: String,
	pub expires_at: chrono::DateTime<chrono::Utc>,
	pub access_token: String,
	pub refresh_token: String
}

impl UserConnection {
	pub fn display(&self) -> String {
		let sub = &self.sub;
		let name = self.display_name.clone().unwrap_or("Unknown".into());
		let username = self.username.clone().unwrap_or("@unknown".into());
		match self.kind {
			UserConnectionKind::Discord => format!("<:discord:1137058089980416080> Discord — [{name}](https://discord.com/users/{sub})"),
			UserConnectionKind::GitHub => format!("<:github:1143983126792642661> GitHub — [{name}](https://github.com/{username})"),
			UserConnectionKind::Roblox => format!("<:roblox:1175038688271536169> Roblox — [{name}](https://www.roblox.com/users/{sub})"),
			UserConnectionKind::YouTube => "placeholder".into(),
			UserConnectionKind::Patreon => format!("<:Patreon:1219706758742933586> Patreon — [{name}](https://www.patreon.com/user?u={sub})"),
		}
	}
}

#[derive(Deserialize, Clone, Debug)]
pub struct UserResponse {
	pub sub: String,
	pub user: User
}

pub async fn get_user_by_discord(id: impl Into<String>, server_id: impl Into<String>) -> Result<Option<UserResponse>> {
	Ok(get_users_by_discord(vec![id.into()], server_id).await?.into_iter().next())
}

pub async fn get_users_by_discord(ids: Vec<String>, server_id: impl Into<String>) -> Result<Vec<UserResponse>> {
	Ok(serde_json::from_str(&DATABASE.from("user_connections")
		.select("sub,user:users(id,server_settings:mellow_user_server_settings(user_connections),connections:user_connections(id,sub,type,username,display_name,oauth_authorisations:user_connection_oauth_authorisations(token_type,expires_at,access_token,refresh_token)))")
		.in_("sub", ids)
		.eq("users.mellow_user_server_settings.server_id", server_id.into())
		.execute().await?.text().await?
	)?)
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
			ProfileSyncActionRequirementKind::PatreonHaveCampaignTier => Some(UserConnectionKind::Patreon),
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
	GitHubInOrganisation,
	PatreonHaveCampaignTier
}

#[derive(Clone, Debug, Deserialize_repr, Serialize_repr)]
#[repr(u8)]
pub enum ProfileSyncActionRequirementsKind {
	MeetAll,
	MeetOne
}

pub async fn get_server_event_response_tree(server_id: impl Into<String>, kind: DocumentKind) -> Result<Document> {
	let server_id = server_id.into();
	let cache_key = (server_id.clone(), kind.clone());
	Ok(match CACHES.event_responses.get(&cache_key)
		.instrument(info_span!("cache.event_responses.read", ?cache_key))
		.await {
			Some(x) => x,
			None => {
				let document: Document = serde_json::from_str(&DATABASE.from("visual_scripting_documents")
					.select("id,name,kind,definition")
					.eq("kind", kind.to_string())
					.eq("mellow_server_id", server_id)
					.limit(1)
					.single()
					.execute()
					.await?
					.text()
					.await?
				)?;
				let span = info_span!("cache.event_responses.write", ?cache_key);
				CACHES.event_responses.insert(cache_key, document.clone())
					.instrument(span)
					.await;

				document
			}
		}
	)
}

pub async fn server_exists(id: impl Into<String>) -> Result<bool> {
	// this isn't an ideal method, but this rust library is way too limited, especially when compared to postgrest-js...
	Ok(DATABASE.from("mellow_servers")
		.select("id")
		.eq("id", id.into())
		.limit(1)
		.single()
		.execute()
		.await?
		.status()
		.is_success()
	)
}