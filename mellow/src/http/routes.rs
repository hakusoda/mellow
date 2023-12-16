use serde::Deserialize;
use actix_web::{ get, web, post, Responder, HttpRequest, HttpResponse };
use derive_more::{ Error, Display };
use ed25519_dalek::{ Verifier, Signature, VerifyingKey };

use crate::{
	discord::get_member,
	syncing::{ SyncMemberResult, SIGN_UPS, sync_single_user },
	database::get_users_by_discord,
	interaction::handle_request
};

const API_KEY: &str = env!("API_KEY");
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
		.service(index)
		.service(interactions)
		.service(sync_member);
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

#[derive(Deserialize)]
struct SyncMemberPayload {
	is_sign_up: Option<bool>,
	webhook_token: Option<String>
}

#[derive(Debug, Display, Error)]
#[display(fmt = "API Error: {}", error)]
struct ApiError {
	error: &'static str
}

impl actix_web::error::ResponseError for ApiError {}

type ApiResult<T> = actix_web::Result<web::Json<T>, ApiError>;

#[post("/server/{server_id}/member/{user_id}/sync")]
async fn sync_member(request: HttpRequest, body: web::Json<SyncMemberPayload>, path: web::Path<(String, String)>) -> ApiResult<SyncMemberResult> {
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		let (server_id, user_id) = path.into_inner();
		if let Some(user) = get_users_by_discord(vec![user_id.clone()], server_id.clone()).await.into_iter().next() {
			let member = get_member(&server_id, &user_id).await;
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				crate::commands::syncing::sync_with_token(user, member, &server_id, &token).await
			} else if body.is_sign_up.is_some_and(|x| x) {
				let result = if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == user_id && x.guild_id == server_id) {
					Some(crate::commands::syncing::sync_with_token(user, member, &server_id, &item.interaction_token).await)
				} else { None };
				SIGN_UPS.write().await.retain(|x| x.user_id != user_id);

				return result.map(|x| web::Json(x)).ok_or(ApiError { error: "sign_up_not_found" });
			} else {
				sync_single_user(&user, &member, server_id).await
			}));
		}
		Err(ApiError { error: "user_not_found" })
	} else { Err(ApiError { error: "invalid_api_key" }) }
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