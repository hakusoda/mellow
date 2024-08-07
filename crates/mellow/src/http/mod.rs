use actix_web::{
	http::{ StatusCode, header::ContentType },
	middleware::Logger,
	App, HttpServer, HttpResponse
};
use derive_more::{ Error, Display };

pub mod routes;

#[tracing::instrument]
pub async fn initialise() -> std::io::Result<()> {
	tokio::spawn(
		HttpServer::new(|| App::new()
			.wrap(Logger::new("%r  →  %s, %b bytes, took %Dms"))
			.configure(routes::configure)
		)
		.bind(("127.0.0.1", 8080))?
		.run()
	);
	Ok(())
}

#[derive(Debug, Display, Error)]
pub enum ApiError {
	#[display(fmt = "cache {}", _0)]
	Cache(mellow_cache::Error),

	#[display(fmt = "model {}", _0)]
	Model(mellow_models::Error),

	#[display(fmt = "invalid_request")]
	GenericInvalidRequest,

	#[display(fmt = "invalid_api_key")]
	InvalidApiKey,

	#[display(fmt = "user_not_found")]
	UserNotFound,

	#[display(fmt = "unknown {}", _0)]
	Unknown(crate::error::Error)
}

impl actix_web::error::ResponseError for ApiError {
	fn error_response(&self) -> HttpResponse {
		HttpResponse::build(self.status_code())
			.insert_header(ContentType::json())
			.body(format!(r#"{{
				"error": "{}"
			}}"#, self))
	}

	fn status_code(&self) -> StatusCode {
		match *self {
			ApiError::Cache(_) |
			ApiError::Model(_) |
			ApiError::Unknown(_) => StatusCode::INTERNAL_SERVER_ERROR,
			ApiError::GenericInvalidRequest => StatusCode::BAD_REQUEST,
			ApiError::InvalidApiKey => StatusCode::FORBIDDEN,
			ApiError::UserNotFound => StatusCode::NOT_FOUND
		}
	}
}

impl From<crate::error::Error> for ApiError {
	fn from(value: crate::error::Error) -> Self {
		Self::Unknown(value)
	}
}

impl From<mellow_cache::Error> for ApiError {
	fn from(value: mellow_cache::Error) -> Self {
		Self::Cache(value)
	}
}

impl From<mellow_models::Error> for ApiError {
	fn from(value: mellow_models::Error) -> Self {
		Self::Model(value)
	}
}

pub type ApiResult<T> = actix_web::Result<T, ApiError>;