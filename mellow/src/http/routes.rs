use sha2::Sha256;
use hmac::{ Mac, Hmac };
use serde::{ Serialize, Deserialize };
use once_cell::sync::Lazy;
use actix_web::{
	Responder, HttpRequest, HttpResponse,
	get, web, post
};
use ed25519_dalek::{ Verifier, Signature, VerifyingKey };
use twilight_model::id::{
	marker::{ UserMarker, GuildMarker },
	Id
};

use super::{ ApiError, ApiResult };
use crate::{
	fetch,
	server::{
		logging::ServerLog,
		action_log::ActionLog,
		Server
	},
	discord::{ GuildMember, APP_ID },
	syncing::{
		sign_ups::SIGN_UPS,
		PatreonPledge, SyncMemberResult, ConnectionMetadata,
		sync_single_user
	},
	database::{
		UserConnectionKind,
		self
	},
	commands::{
		syncing::sync_with_token,
		COMMANDS
	},
	interaction,
	Result
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
		.service(update_discord_commands)
		.service(patreon_webhook)
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
    interaction::handle_request(body).await.map_err(|x| { println!("{x}"); x })
}

#[derive(Deserialize)]
struct SyncMemberPayload {
	is_sign_up: Option<bool>,
	webhook_token: Option<String>
}

#[post("/server/{server_id}/member/{user_id}/sync")]
async fn sync_member(request: HttpRequest, body: web::Json<SyncMemberPayload>, path: web::Path<(Id<GuildMarker>, Id<UserMarker>)>) -> ApiResult<web::Json<SyncMemberResult>> {
	// TODO: make this... easier on the eyes.
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		let (guild_id, user_id) = path.into_inner();
		if let Some(user) = database::get_user_by_discord(&guild_id, &user_id).await? {
			let member = GuildMember::fetch(&guild_id, &user_id).await?;
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				sync_with_token(user, member, &guild_id, &token, false).await?
			} else if body.is_sign_up.is_some_and(|x| x) {
				let result = if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == user_id && x.guild_id == guild_id) {
					Some(sync_with_token(user, member, &guild_id, &item.interaction_token, true).await?)
				} else { None };
				SIGN_UPS.write().await.retain(|x| x.user_id != user_id);

				return result.map(|x| web::Json(x)).ok_or(ApiError::SignUpNotFound);
			} else {
				sync_single_user(&user, &member, &guild_id, None).await?
			}));
		}
		Err(ApiError::UserNotFound)
	} else { Err(ApiError::InvalidApiKey) }
}

#[post("/supabase_webhooks/action_log")]
async fn action_log_webhook(request: HttpRequest, body: String) -> ApiResult<HttpResponse> {
	absolutesolver(&request, &body)?;

	let payload: ActionLog = serde_json::from_str(&body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	Server::fetch(&payload.server_id)
		.await?
		.send_logs(vec![ServerLog::ActionLog(payload)])
		.await?;

	Ok(HttpResponse::Ok().finish())
}

#[derive(Deserialize)]
struct PayloadData<T> {
	data: T
}

#[derive(Deserialize)]
struct WebhookPayload {
	attributes: Attributes,
	relationships: PayloadRelationships
}

#[derive(Deserialize)]
struct Attributes {
	patron_status: Option<String>,
}

#[derive(Deserialize)]
struct PayloadRelationships {
	user: PayloadData<IdContainer>,
	campaign: PayloadData<IdContainer>,
	currently_entitled_tiers: PayloadData<Vec<IdContainer>>
}

#[derive(Deserialize)]
struct IdContainer {
	id: String
}

#[post("/patreon_webhook")]
async fn patreon_webhook(body: String) -> ApiResult<HttpResponse> {
	println!("{body}");
	let payload: PayloadData<WebhookPayload> = serde_json::from_str(&body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	let response: serde_json::Value = serde_json::from_str(&database::DATABASE.from("mellow_server_oauth_authorisations")
		.select("server_id")
		.eq("patreon_campaign_id", &payload.data.relationships.campaign.data.id)
		.execute().await.unwrap().text().await.unwrap()
	).map_err(|_| ApiError::GenericInvalidRequest)?;

	let user_id = &payload.data.relationships.user.data.id;
	let guild_id: Id<GuildMarker> = serde_json::from_value(response.get("server_id").unwrap().clone())
		.map_err(|_| ApiError::GenericInvalidRequest)?;
	if let Some(user) = database::get_user_by_discord(&guild_id, &Id::new(user_id.parse().map_err(|_| ApiError::GenericInvalidRequest)?)).await? {
		let member = GuildMember::fetch(&guild_id, &Id::new(user.user.connections.iter().find(|x| matches!(x.kind, UserConnectionKind::Discord)).unwrap().id.parse().map_err(|_| ApiError::GenericInvalidRequest)?)).await?;
		sync_single_user(&user, &member, &guild_id, Some(ConnectionMetadata {
			patreon_pledges: vec![PatreonPledge {
				tiers: payload.data.relationships.currently_entitled_tiers.data.iter().map(|x| x.id.clone()).collect(),
				active: payload.data.attributes.patron_status.map_or(false, |x| x == "active_patron"),
				user_id: user.user.id.clone(),
				campaign_id: payload.data.relationships.campaign.data.id.clone(),
				connection_id: user.user.server_connections().into_iter().find(|x| matches!(x.kind, database::UserConnectionKind::Patreon)).unwrap().id.clone()
			}],
			roblox_memberships: vec![]
		})).await?;
	}

	Ok(HttpResponse::Ok().finish())
}

fn absolutesolver(request: &HttpRequest, body: impl ToString) -> Result<()> {
	let mut mac = HmacSha256::new_from_slice(ABSOLUTESOLVER)?;
	mac.update(body.to_string().as_bytes());

	Ok(mac.verify_slice(
		request.headers()
			.get("absolutesolver")
			.map(|x| hex::decode(x))
			.unwrap()?
			.as_slice()
	)?)
}

#[derive(Serialize)]
struct ApplicationCommand {
	name: String,
	description: String,
	#[serde(skip_serializing_if = "Option::is_none")]
	dm_permission: Option<bool>,
	#[serde(skip_serializing_if = "Option::is_none")]
	default_member_permissions: Option<String>
}

#[post("/update_discord_commands")]
async fn update_discord_commands(request: HttpRequest) -> ApiResult<HttpResponse> {
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		fetch::CLIENT.put(format!("https://discord.com/api/v10/applications/{APP_ID}/commands"))
			.json(&COMMANDS.iter().map(|x| ApplicationCommand {
				name: x.name.to_string(),
				description: x.description.clone().unwrap_or("there is no description yet, how sad...".into()),
				dm_permission: Some(!x.no_dm),
				default_member_permissions: x.default_member_permissions.clone()
			}).collect::<Vec<ApplicationCommand>>())
			.header("content-type", "application/json")
			.send()
			.await
			.unwrap();
		Ok(HttpResponse::Ok().finish())
	} else { Err(ApiError::InvalidApiKey) }
}

fn verify_interaction_body(body: impl Into<String>, signature: impl Into<String>, timestamp: impl Into<String>) -> Result<()> {
	Ok(PUBLIC_KEY.verify(
		format!("{}{}", timestamp.into(), body.into()).as_bytes(),
		&hex::decode(signature.into())
			.map(|vec| Signature::from_bytes(&vec.try_into().unwrap()))
			.unwrap()
	)?)
}