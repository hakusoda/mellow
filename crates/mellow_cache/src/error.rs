use mellow_models::hakumi::user::connection::ConnectionKind;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Model: {0}")]
	Model(#[from] mellow_models::Error),

	#[error("Model not found")]
	ModelNotFound,

	#[error("Reqwest: {0}")]
	Reqwest(#[from] reqwest::Error),

	#[error("Serde JSON: {0}")]
	SerdeJson(#[from] serde_json::Error),

	#[error("SIMD JSON: {0}")]
	SimdJson(#[from] simd_json::Error),
	
	#[error("SQLx: {0}")]
	Sqlx(#[from] sqlx::Error),

	#[error("Your {0:?} connection appears to be invalid, please reconnect it [here](<https://hakumi.cafe/settings/account/connections>).")]
	UserConnectionInvalid(ConnectionKind),

	#[error("Failed to refresh user connection")]
	UserConnectionRefresh
}

pub type Result<T> = core::result::Result<T, Error>;