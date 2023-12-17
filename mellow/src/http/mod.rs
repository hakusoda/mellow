use actix_web::{
	web,
	http::{ StatusCode, header::ContentType },
	middleware::Logger,
	App, HttpServer, HttpResponse
};
use derive_more::{ Error, Display };

mod routes;

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
	NotImplemented
}

impl actix_web::error::ResponseError for ApiError {
	fn error_response(&self) -> HttpResponse {
		HttpResponse::build(self.status_code())
			.insert_header(ContentType::json())
			.body(format!(r#"
				"error": "{}"
			"#, self.to_string()))
	}

	fn status_code(&self) -> StatusCode {
		match *self {
			ApiError::GenericInvalidRequest => StatusCode::BAD_REQUEST,
			ApiError::InvalidApiKey |
			ApiError::InvalidSignature => StatusCode::FORBIDDEN,
			ApiError::UserNotFound |
			ApiError::SignUpNotFound => StatusCode::NOT_FOUND,
			ApiError::NotImplemented => StatusCode::NOT_IMPLEMENTED
		}
	}
}

pub type ApiResult<T> = actix_web::Result<web::Json<T>, ApiError>;