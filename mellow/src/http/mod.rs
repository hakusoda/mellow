use actix_web::{
	http::{ StatusCode, header::ContentType },
	middleware::Logger,
	App, HttpServer, HttpResponse
};
use derive_more::{ Error, Display };

pub mod routes;

pub async fn start() -> std::io::Result<()> {
	HttpServer::new(||
		App::new()
			.wrap(Logger::new("%r  →  %s, %b bytes, took %Dms"))
			.configure(routes::configure)
	)
		.bind(("127.0.0.1", 8080))?
		.run()
		.await
}

#[derive(Debug, Display, Error)]
pub enum ApiError {
	#[display(fmt = "internal_error")]
	InternalError,

	#[display(fmt = "invalid_request")]
	GenericInvalidRequest,

	#[display(fmt = "invalid_api_key")]
	InvalidApiKey,

	#[display(fmt = "invalid_signature")]
	InvalidSignature,

	#[display(fmt = "user_not_found")]
	UserNotFound,

	#[display(fmt = "sign_up_not_found")]
	SignUpNotFound,

	#[display(fmt = "not_implemented")]
	NotImplemented,

	#[display(fmt = "unknown")]
	Unknown
}

impl actix_web::error::ResponseError for ApiError {
	fn error_response(&self) -> HttpResponse {
		HttpResponse::build(self.status_code())
			.insert_header(ContentType::json())
			.body(format!(r#"{{
				"error": "{}"
			}}"#, self.to_string()))
	}

	fn status_code(&self) -> StatusCode {
		match *self {
			ApiError::Unknown |
			ApiError::InternalError => StatusCode::INTERNAL_SERVER_ERROR,
			ApiError::GenericInvalidRequest => StatusCode::BAD_REQUEST,
			ApiError::InvalidApiKey |
			ApiError::InvalidSignature => StatusCode::FORBIDDEN,
			ApiError::UserNotFound |
			ApiError::SignUpNotFound => StatusCode::NOT_FOUND,
			ApiError::NotImplemented => StatusCode::NOT_IMPLEMENTED
		}
	}
}

impl From<crate::error::Error> for ApiError {
	fn from(_value: crate::error::Error) -> Self {
		Self::Unknown
	}
}

pub type ApiResult<T> = actix_web::Result<T, ApiError>;