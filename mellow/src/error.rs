use tracing_error::{ SpanTrace, InstrumentError };

#[derive(Debug, thiserror::Error)]
pub enum ErrorKind {
	#[error("API Error: {0}")]
	ApiError(#[from] crate::http::ApiError),

	#[error("HTTP Error: {0}")]
	HttpError(#[from] reqwest::Error),

	#[error("HTTP Error: {0} {1}")]
	FormattedHttpError(String, String),

	#[error("Mac Error: {0}")]
	MacError(#[from] hmac::digest::MacError),

	#[error("JSON Error: {0}")]
	JsonError(#[from] serde_json::Error),

	#[error("Signature Error: {0}")]
	SignatureError(#[from] ed25519_dalek::SignatureError)
}

#[derive(Debug)]
pub struct Error {
    pub source: tracing_error::TracedError<ErrorKind>,
	pub context: SpanTrace
}

impl std::fmt::Display for Error {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{}", self.source)
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.source.source()
    }
}

impl<E: Into<ErrorKind>> From<E> for Error {
    fn from(source: E) -> Self {
        Self {
            source: Into::<ErrorKind>::into(source).in_current_span(),
			context: SpanTrace::capture()
        }
    }
}

pub type Result<T> = core::result::Result<T, Error>;