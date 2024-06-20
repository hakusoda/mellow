use tracing_error::SpanTrace;

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
	#[error("HTTP Error: {0}")]
	HttpError(#[from] reqwest::Error),

	#[error("Url Parse Error: {0}")]
	UrlParseError(#[from] url::ParseError),

	#[error("HTTP Error: {0} {1}")]
	FormattedHttpError(String, String),

	#[error("Discord Error: {0}")]
	TwilightHttpError(#[from] twilight_http::Error),

	#[error("Discord Validation Error: {0}")]
	TwilightValidationError(#[from] twilight_validate::request::ValidationError),

	#[error("Discord Channel Validation Error: {0}")]
	TwilightChannelValidationError(#[from] twilight_validate::channel::ChannelValidationError),

	#[error("Discord Message Validation Error: {0}")]
	TwilightMessageValidationError(#[from] twilight_validate::message::MessageValidationError),

	#[error("Discord Deserialisation Error: {0}")]
	TwilightDeserialiseError(#[from] twilight_http::response::DeserializeBodyError),

	#[error("Timestamp Error: {0}")]
	TwilightTimestampError(#[from] twilight_model::util::datetime::TimestampParseError),
	#[error("Image Source Url Error: {0}")]
	TwilightImageUrlError(#[from] twilight_util::builder::embed::image_source::ImageSourceUrlError),

	#[error("Twilight Channel Error: {0}")]
	TwilightSendError(#[from] twilight_gateway::error::ChannelError),

	#[error("User Ids Error: {0}")]
	TwilightUserIdsError(#[from] twilight_model::gateway::payload::outgoing::request_guild_members::UserIdsError),

	#[error("OneShot Receive Error: {0}")]
	OneshotReceiveError(#[from] tokio::sync::oneshot::error::RecvError),

	#[error("Mac Error: {0}")]
	MacError(#[from] hmac::digest::MacError),

	#[error("JSON Error: {0}")]
	JsonError(#[from] serde_json::Error),
	#[error("SIMD Error: {0}")]
	SimdError(#[from] simd_json::Error),
	#[error("System Time Error: {0}")]
	SystemTimeError(#[from] std::time::SystemTimeError),

	#[error("SQLx Error: {0}")]
	SqlxError(#[from] sqlx::Error),

	#[error("Integer Parsing Error: {0}")]
	ParseIntegerError(#[from] std::num::ParseIntError),

	#[error("Hex Error: {0}")]
	HexError(#[from] hex::FromHexError),

	#[error("Sha2 Invalid Length Error: {0}")]
	InvalidLengthError(#[from] sha2::digest::InvalidLength),

	#[error("PostgREST Error: {0}")]
	PostgrestError(#[from] postgrest::Error),

	#[error("Unknown Error")]
	Unknown
}

#[derive(Debug)]
pub struct Error {
	pub kind: ErrorKind,
	pub context: SpanTrace
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.kind)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.kind.source()
    }
}

impl<E: Into<ErrorKind>> From<E> for Error {
    fn from(source: E) -> Self {
        Self {
			kind: Into::<ErrorKind>::into(source),
			context: SpanTrace::capture()
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;