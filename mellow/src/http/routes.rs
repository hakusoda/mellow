use sha2::Sha256;
use hmac::{ Mac, Hmac };
use serde::Deserialize;
use actix_web::{
	Responder, HttpRequest, HttpResponse,
	get, web, post
};
use twilight_util::builder::command::CommandBuilder;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	application::command::{ Command, CommandType }
};

use super::{ ApiError, ApiResult };
use crate::{
	model::{
		discord::DISCORD_MODELS,
		hakumi::{
			user::connection::ConnectionKind,
			HAKUMI_MODELS
		},
		mellow::MELLOW_MODELS
	},
	traits::{ WithId, Partial },
	server::{
		logging::ServerLog,
		action_log::ActionLog,
	},
	discord::INTERACTION,
	syncing::{
		sign_ups::SIGN_UPS,
		PatreonPledge, SyncMemberResult, ConnectionMetadata,
		sync_single_user
	},
	database,
	commands::{
		syncing::sync_with_token,
		COMMANDS
	},
	Result
};

type HmacSha256 = Hmac<Sha256>;

const API_KEY: &str = env!("API_KEY");
const ABSOLUTESOLVER: &[u8] = env!("ABSOLUTESOLVER").as_bytes();
const CARGO_PKG_VERSION: &str = env!("CARGO_PKG_VERSION");

pub fn configure(cfg: &mut web::ServiceConfig) {
    cfg
		.service(index)
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
		if let Some(user) = HAKUMI_MODELS.user_by_discord(guild_id, user_id).await? {
			let member = DISCORD_MODELS.member(guild_id, user_id).await?.partial().with_id(user_id);
			let server = MELLOW_MODELS.server(guild_id).await?;
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				sync_with_token(server.value(), user.value(), member, &token, false, None).await?
			} else if body.is_sign_up.is_some_and(|x| x) {
				let result = if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == user_id && x.guild_id == guild_id) {
					Some(sync_with_token(server.value(), user.value(), member, &item.interaction_token, true, None).await?)
				} else { None };
				SIGN_UPS.write().await.retain(|x| x.user_id != user_id);

				return result.map(|x| web::Json(x)).ok_or(ApiError::SignUpNotFound);
			} else {
				sync_single_user(server.value(), user.value(), &member, None).await?
			}));
		}
		Err(ApiError::UserNotFound)
	} else { Err(ApiError::InvalidApiKey) }
}

#[post("/supabase_webhooks/action_log")]
async fn action_log_webhook(request: HttpRequest, payload: web::Payload) -> ApiResult<HttpResponse> {
	let mut body = payload.to_bytes().await.unwrap().to_vec();
	absolutesolver(&request, &body)?;

	let payload: ActionLog = simd_json::from_slice(&mut body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	MELLOW_MODELS.server(payload.server_id)
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
async fn patreon_webhook(payload: web::Payload) -> ApiResult<HttpResponse> {
	let mut body = payload.to_bytes().await.unwrap().to_vec();
	let payload: PayloadData<WebhookPayload> = simd_json::from_slice(&mut body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	let response: serde_json::Value = simd_json::from_slice(&mut database::DATABASE.from("mellow_server_oauth_authorisations")
		.select("server_id")
		.eq("patreon_campaign_id", &payload.data.relationships.campaign.data.id)
		.execute()
		.await.unwrap()
		.bytes()
		.await.unwrap()
		.to_vec()
	).map_err(|_| ApiError::GenericInvalidRequest)?;

	let user_id = &payload.data.relationships.user.data.id;
	let guild_id: Id<GuildMarker> = serde_json::from_value(response.get("server_id").unwrap().clone())
		.map_err(|_| ApiError::GenericInvalidRequest)?;
	if let Some(user) = HAKUMI_MODELS.user_by_discord(guild_id, Id::new(user_id.parse().map_err(|_| ApiError::GenericInvalidRequest)?)).await? {
		let discord_id = Id::new(user.connections.iter().find(|x| matches!(x.kind, ConnectionKind::Discord)).unwrap().id.to_string().parse().map_err(|_| ApiError::GenericInvalidRequest)?);
		let member = DISCORD_MODELS.member(guild_id, discord_id).await?.partial().with_id(discord_id);
		let server = MELLOW_MODELS.server(guild_id).await?;
		sync_single_user(server.value(), user.value(), &member, Some(ConnectionMetadata {
			patreon_pledges: vec![PatreonPledge {
				tiers: payload.data.relationships.currently_entitled_tiers.data.iter().map(|x| x.id.clone()).collect(),
				active: payload.data.attributes.patron_status.map_or(false, |x| x == "active_patron"),
				user_id: user.id.clone(),
				campaign_id: payload.data.relationships.campaign.data.id.clone(),
				connection_id: user.server_connections().into_iter().find(|x| matches!(x.kind, ConnectionKind::Patreon)).unwrap().id.clone()
			}],
			roblox_memberships: vec![]
		})).await?;
	}

	Ok(HttpResponse::Ok().finish())
}

fn absolutesolver(request: &HttpRequest, body: &[u8]) -> Result<()> {
	let mut mac = HmacSha256::new_from_slice(ABSOLUTESOLVER)?;
	mac.update(body);

	Ok(mac.verify_slice(
		request.headers()
			.get("absolutesolver")
			.map(|x| hex::decode(x))
			.unwrap()?
			.as_slice()
	)?)
}

fn app_command(command: &crate::Command, kind: CommandType) -> Result<Command> {
	let description = match kind {
		CommandType::User => "",
		_ => command.description.as_ref().map_or("there is no description yet, how sad...", |x| x.as_str())
	};
	let mut builder = CommandBuilder::new(&command.name, description, kind)
		.dm_permission(!command.no_dm);
	if let Some(permissions) = command.default_member_permissions()? {
		builder = builder.default_member_permissions(permissions);
	}
	Ok(builder.build())
}

#[post("/update_discord_commands")]
async fn update_discord_commands(request: HttpRequest) -> ApiResult<HttpResponse> {
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY.to_string()) {
		let mut commands: Vec<Command> = vec![];
		for command in COMMANDS.iter() {
			if command.is_user {
				commands.push(app_command(&command, CommandType::User)?);
			}
			if command.is_slash {
				commands.push(app_command(&command, CommandType::ChatInput)?);
			}
			if command.is_message {
				commands.push(app_command(&command, CommandType::Message)?);
			}
		}

		INTERACTION.set_global_commands(&commands).await.unwrap();
		Ok(HttpResponse::Ok().finish())
	} else { Err(ApiError::InvalidApiKey) }
}