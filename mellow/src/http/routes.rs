use actix_web::{ get, web, post, Responder, HttpRequest, HttpResponse };
use ed25519_dalek::{ Verifier, Signature, VerifyingKey };

use crate::{
	syncing::finish_sign_up,
	interaction::handle_request
};

const API_KEY: &str = env!("API_KEY");
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
		.service(index)
		.service(interactions)
		.service(signup_finished);
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(format!("hello from mellow v{CARGO_PKG_VERSION}!\nhttps://github.com/hakusoda/mellow"))
}

#[post("/interactions")]
async fn interactions(request: HttpRequest, body: String) -> impl Responder {
	let headers = request.headers();
	let body = parse_body(
		body,
		headers.get("x-signature-ed25519").unwrap().to_str().unwrap(),
		headers.get("x-signature-timestamp").unwrap().to_str().unwrap()
	);
    handle_request(body).await
}

#[post("/signup-finished")]
async fn signup_finished(request: HttpRequest, body: String) -> impl Responder {
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		finish_sign_up(body).await;
	}
	HttpResponse::Ok()
}

fn parse_body(body: String, signature: &str, timestamp: &str) -> String {
	let public_key = hex::decode(env!("DISCORD_PUBLIC_KEY"))
        .map(|vec| VerifyingKey::from_bytes(&vec.try_into().unwrap()).unwrap())
		.unwrap();
	public_key.verify(
        format!("{}{}", timestamp, body).as_bytes(),
        &hex::decode(signature)
            .map(|vec| Signature::from_bytes(&vec.try_into().unwrap()))
			.unwrap()
    ).unwrap();
	body
}