use chrono::{ DateTime, Utc };
use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct OAuthAuthorisationModel {
	pub id: u64,
	pub expires_at: DateTime<Utc>,
	pub access_token: String,
	pub refresh_token: String,
	pub token_type: String
}