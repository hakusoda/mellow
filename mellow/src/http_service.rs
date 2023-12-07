use std::convert::Infallible;
use hyper::{
	body::Bytes,
	Method, Request, Response, StatusCode
};
use http_body_util::{ combinators::BoxBody, Full, Empty, BodyExt };

use crate::{
	syncing::finish_sign_up,
	interaction::handle_request
};

static API_KEY: &'static str = std::env!("API_KEY");

pub async fn service(request: Request<hyper::body::Incoming>) -> Result<Response<BoxBody<Bytes, hyper::Error>>, Infallible> {
	println!("{} {}", request.method(), request.uri());
	match (request.method(), request.uri().path()) {
		(&Method::GET, "/") => Ok(Response::new(full(
			"AHHHHHHHHHH!!!!!!!!!!!! rust.",
		))),
		(&Method::POST, "/interactions") => {
			let body = handle_request(request).await;
			let response = Response::builder()
				.header("content-type", "application/json")
				.body(body)
				.unwrap();
			Ok(response)
		},
		(&Method::POST, "/signup-finished") => {
			if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
				finish_sign_up(String::from_utf8(request.collect().await.unwrap().to_bytes().to_vec()).unwrap()).await;
			}
			Ok(Response::new(empty()))
		},
		_ => {
			let mut not_found = Response::new(empty());
			*not_found.status_mut() = StatusCode::NOT_FOUND;
			Ok(not_found)
		}
	}
}

pub fn empty() -> BoxBody<Bytes, hyper::Error> {
	Empty::<Bytes>::new()
		.map_err(|never| match never {})
		.boxed()
}

pub fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
	Full::new(chunk.into())
		.map_err(|never| match never {})
		.boxed()
}

pub fn json<T: Sized + serde::Serialize>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
	full(serde_json::to_string(&chunk).unwrap())
}