#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Serde JSON: {0}")]
	SerdeJson(#[from] serde_json::Error),
	
	#[error("SQLx: {0}")]
	Sqlx(#[from] sqlx::Error),

	#[error("Twilight HTTP: {0}")]
	TwilightHttp(#[from] twilight_http::Error),

	#[error("Twilight HTTP Deserialise Body: {0}")]
	TwilightHttpDeserialiseBody(#[from] twilight_http::response::DeserializeBodyError)
}

pub type Result<T> = core::result::Result<T, Error>;