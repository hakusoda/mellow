use sha2::Sha256;
use hmac::{ Mac, Hmac };
use serde::{ Serialize, Deserialize };
use once_cell::sync::Lazy;
use actix_web::{
	Responder, HttpRequest, HttpResponse,
	get, web, post
};
use ed25519_dalek::{ Verifier, Signature, VerifyingKey, SignatureError };

use super::{ ApiError, ApiResult };
use crate::{
	server::ServerLog,
	discord::get_member,
	syncing::{ SyncMemberResult, SIGN_UPS, sync_single_user },
	database,
	interaction
};

type HmacSha256 = Hmac<Sha256>;

const API_KEY: &str = env!("API_KEY");
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

const PUBLIC_KEY: Lazy<VerifyingKey> = Lazy::new(||
	hex::decode(env!("DISCORD_PUBLIC_KEY"))
		.map(|vec| VerifyingKey::from_bytes(&vec.try_into().unwrap()).unwrap())
		.unwrap()
);
const ABSOLUTESOLVER: &[u8] = env!("ABSOLUTESOLVER").as_bytes();

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
		.service(index)
		.service(interactions)
		.service(sync_member)
		.service(
			web::scope("/absolutesolver")
				.service(action_log_webhook)
		);
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(format!("hello from mellow v{CARGO_PKG_VERSION}!\nhttps://github.com/hakusoda/mellow"))
}

#[post("/interactions")]
async fn interactions(request: HttpRequest, body: String) -> ApiResult<web::Json<interaction::InteractionResponse>> {
	let headers = request.headers();
	let signature = headers.get("x-signature-ed25519")
		.and_then(|x| x.to_str().ok())
		.ok_or_else(|| ApiError::GenericInvalidRequest)?;
	let timestamp = headers.get("x-signature-timestamp")
		.and_then(|x| x.to_str().ok())
		.ok_or_else(|| ApiError::GenericInvalidRequest)?;

	// here we verify that the request originated from Discord with cryptography
	if let Err(_) = verify_interaction_body(&body, signature, timestamp) {
		return Err(ApiError::InvalidSignature);
	}
    interaction::handle_request(body).await
}

#[derive(Deserialize)]
struct SyncMemberPayload {
	is_sign_up: Option<bool>,
	webhook_token: Option<String>
}

#[post("/server/{server_id}/member/{user_id}/sync")]
async fn sync_member(request: HttpRequest, body: web::Json<SyncMemberPayload>, path: web::Path<(String, String)>) -> ApiResult<web::Json<SyncMemberResult>> {
	// TODO: make this... easier on the eyes.
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		let (server_id, user_id) = path.into_inner();
		if let Some(user) = database::get_users_by_discord(vec![user_id.clone()], server_id.clone()).await.into_iter().next() {
			let member = get_member(&server_id, &user_id).await;
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				crate::commands::syncing::sync_with_token(user, member, &server_id, &token).await
			} else if body.is_sign_up.is_some_and(|x| x) {
				let result = if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == user_id && x.guild_id == server_id) {
					Some(crate::commands::syncing::sync_with_token(user, member, &server_id, &item.interaction_token).await)
				} else { None };
				SIGN_UPS.write().await.retain(|x| x.user_id != user_id);

				return result.map(|x| web::Json(x)).ok_or(ApiError::SignUpNotFound);
			} else {
				sync_single_user(&user, &member, server_id).await
			}));
		}
		Err(ApiError::UserNotFound)
	} else { Err(ApiError::InvalidApiKey) }
}

#[derive(Deserialize, Serialize)]
pub struct ActionLogAuthor {
	pub id: String,
	pub name: Option<String>,
	pub username: String
}

impl ActionLogAuthor {
	pub fn display_name(&self) -> String {
		self.name.as_ref().map_or_else(|| self.username.clone(), |x| x.clone())
	}
}

#[derive(Deserialize, Serialize)]
pub struct ActionLogWebhookPayload {
	#[serde(rename = "type")]
	pub kind: String,
	pub author: ActionLogAuthor,
	pub server_id: String
}

#[post("/supabase_webhooks/action_log")]
async fn action_log_webhook(request: HttpRequest, body: String) -> ApiResult<HttpResponse> {
	absolutesolver(&request, &body)?;

	let payload: ActionLogWebhookPayload = serde_json::from_str(&body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	database::get_server(&payload.server_id)
		.await
		.send_logs(vec![ServerLog::ActionLog(payload)])
		.await;

	Ok(HttpResponse::Ok().finish())
}

fn absolutesolver(request: &HttpRequest, body: impl ToString) -> Result<(), ApiError> {
	let mut mac = HmacSha256::new_from_slice(ABSOLUTESOLVER)
		.map_err(|_| ApiError::InternalError)?;
	mac.update(body.to_string().as_bytes());

	mac.verify_slice(
		request.headers()
			.get("absolutesolver")
			.ok_or(ApiError::InvalidSignature)
			.map(|x| hex::decode(x))?
			.map_err(|_| ApiError::InvalidSignature)?
			.as_slice()
	)
		.map_err(|_| ApiError::InvalidSignature)
}

fn verify_interaction_body(body: impl Into<String>, signature: impl Into<String>, timestamp: impl Into<String>) -> Result<(), SignatureError> {
	PUBLIC_KEY.verify(
		format!("{}{}", timestamp.into(), body.into()).as_bytes(),
		&hex::decode(signature.into())
			.map(|vec| Signature::from_bytes(&vec.try_into().unwrap()))
			.unwrap()
	)
}