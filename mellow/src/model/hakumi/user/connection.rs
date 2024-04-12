use uuid::Uuid;
use serde::{ Serialize, Deserialize };
use serde_repr::{ Serialize_repr, Deserialize_repr };

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Connection {
	pub id: Uuid,
	pub sub: String,
	#[serde(rename = "type")]
	pub kind: ConnectionKind,
	pub username: Option<String>,
	pub display_name: Option<String>,
	pub oauth_authorisations: Option<Vec<ConnectionOAuthAuthorisation>>
}

impl Connection {
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
}

#[derive(Clone, Debug, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum ConnectionKind {
	Discord,
	GitHub,
	Roblox,
	YouTube,
	Patreon
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ConnectionOAuthAuthorisation {
	pub token_type: String,
	pub expires_at: chrono::DateTime<chrono::Utc>,
	pub access_token: String,
	pub refresh_token: String
}