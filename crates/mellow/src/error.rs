#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("Reqwest Error: {0}")]
	Reqwest(#[from] reqwest::Error),

	#[error("Url Parse Error: {0}")]
	UrlParse(#[from] url::ParseError),

	#[error("Fetch Error: {0} {1}")]
	Fetch(String, String),

	#[error("Discord Error: {0}")]
	TwilightHttp(#[from] twilight_http::Error),

	#[error("Discord Deserialisation Error: {0}")]
	TwilightDeserialise(#[from] twilight_http::response::DeserializeBodyError),

	#[error("Timestamp Error: {0}")]
	TwilightTimestamp(#[from] twilight_model::util::datetime::TimestampParseError),
	#[error("Image Source Url Error: {0}")]
	TwilightImageUrl(#[from] twilight_util::builder::embed::image_source::ImageSourceUrlError),

	#[error("Twilight Channel Error: {0}")]
	TwilightChannel(#[from] twilight_gateway::error::ChannelError),

	#[error("User Ids Error: {0}")]
	TwilightUserIds(#[from] twilight_model::gateway::payload::outgoing::request_guild_members::UserIdsError),

	#[error("OneShot Receive Error: {0}")]
	OneshotReceive(#[from] tokio::sync::oneshot::error::RecvError),

	#[error("Mac Error: {0}")]
	Mac(#[from] hmac::digest::MacError),

	#[error("Serde JSON Error: {0}")]
	SerdeJson(#[from] serde_json::Error),
	#[error("SIMD JSON Error: {0}")]
	SimdJson(#[from] simd_json::Error),
	#[error("System Time Error: {0}")]
	SystemTime(#[from] std::time::SystemTimeError),

	#[error("SQLx Error: {0}")]
	Sqlx(#[from] sqlx::Error),

	#[error("Integer Parsing Error: {0}")]
	ParseInteger(#[from] std::num::ParseIntError),

	#[error("FromHex Error: {0}")]
	FromHex(#[from] hex::FromHexError),

	#[error("Sha2 Invalid Length Error: {0}")]
	Sha2InvalidLength(#[from] sha2::digest::InvalidLength),

	#[error("PostgREST Error: {0}")]
	Postgrest(#[from] postgrest::Error)
}

pub type Result<T> = core::result::Result<T, Error>;