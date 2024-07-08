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
		hakumi::{
			id::{
				marker::{ UserMarker as HakuUserMarker, ConnectionMarker },
				HakuId
			},
			user::connection::{ Connection, ConnectionKind, OAuthAuthorisation },
			HAKUMI_MODELS
		},
		mellow::{
			server::{ Server, user_settings::{ UserSettings, ConnectionReference } },
			MELLOW_MODELS
		}
	},
	server::{
		logging::ServerLog,
		action_log::{ ActionLog, DataChange, ActionLogAuthor },
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
	visual_scripting::Document,
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
				.service(model_update_webhook)
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

#[post("/server/{server_id}/member/{member_id}/sync")]
async fn sync_member(request: HttpRequest, body: web::Json<SyncMemberPayload>, path: web::Path<(u64, u64)>) -> ApiResult<web::Json<SyncMemberResult>> {
	// TODO: make this... easier on the eyes.
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY) {
		let (guild_id, member_id) = path.into_inner();
		let guild_id: Id<GuildMarker> = Id::new(guild_id);
		let member_id: Id<UserMarker> = Id::new(member_id);
		if let Some(user_id) = HAKUMI_MODELS.user_by_discord(guild_id, member_id).await? {
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				sync_with_token(guild_id, user_id, member_id, token, false, None).await?
			} else if body.is_sign_up.is_some_and(|x| x) {
				let result = if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == member_id && x.guild_id == guild_id) {
					Some(sync_with_token(guild_id, user_id, member_id, &item.interaction_token, true, None).await?)
				} else { None };
				SIGN_UPS.write().await.retain(|x| x.user_id != member_id);

				return result.map(web::Json).ok_or(ApiError::SignUpNotFound);
			} else {
				sync_single_user(guild_id, user_id, member_id, None).await?
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

#[derive(Debug, Deserialize)]
struct ModelUpdate {
	#[serde(flatten)]
	kind: ModelUpdateKind,
	actionee: Option<ActionLogAuthor>
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename = "snake_case")]
enum ModelUpdateKind {
	#[serde(rename = "mellow_server")]
	Server(Server),
	#[serde(rename = "mellow_user_server_settings")]
	UserServerSettings {
		user_id: HakuId<HakuUserMarker>,
		server_id: Id<GuildMarker>,
		user_connections: Vec<ConnectionReference>
	},
	UserConnection {
		user_id: HakuId<HakuUserMarker>,
		#[serde(flatten)]
		connection: Connection
	},
	#[serde(rename = "user_connection_oauth_authorisation")]
	UserConnectionOAuthAuthorisation {
		user_id: HakuId<HakuUserMarker>,
		connection_id: HakuId<ConnectionMarker>,
		#[serde(flatten)]
		oauth_authorisation: OAuthAuthorisation
	},
	VisualScriptingDocument(Document)
}

#[post("/supabase_webhooks/model_update")]
async fn model_update_webhook(_request: HttpRequest, payload: web::Json<ModelUpdate>) -> ApiResult<HttpResponse> {
	let model_update = payload.into_inner();
	match model_update.kind {
		ModelUpdateKind::Server(new_server) => {
			let id = new_server.id;
			let new_logging_types = new_server.logging_types;
			let new_logging_channel_id = new_server.logging_channel_id;
			if let Some(old_server) = MELLOW_MODELS.servers.insert(id, new_server) {
				let mut logging_data_changes: Vec<DataChange> = vec![];
				if old_server.logging_types != new_logging_types {
					logging_data_changes.push(DataChange::updated("Active Events", old_server.logging_types, new_logging_types)?);
				}
				if old_server.logging_channel_id != new_logging_channel_id {
					if old_server.logging_channel_id.is_none() {
						logging_data_changes.push(DataChange::created("Channel", new_logging_channel_id.unwrap())?);
					} else if new_logging_channel_id.is_none() {
						logging_data_changes.push(DataChange::deleted("Channel", old_server.logging_channel_id.unwrap())?);
					} else {
						logging_data_changes.push(DataChange::updated("Channel", old_server.logging_channel_id.unwrap(), new_logging_channel_id.unwrap())?);
					}
				}

				if !logging_data_changes.is_empty() {
					if let Err(err) = old_server.send_logs(vec![
						ServerLog::ActionLog(ActionLog {
							kind: "mellow.server.discord_logging.updated".into(),
							author: model_update.actionee,
							server_id: id,
							data_changes: logging_data_changes,
							target_command: None,
							target_webhook: None,
							target_document: None,
							target_sync_action: None
						})
					]).await {
						println!("{err}");
					}
				}
			}
			println!("model::mellow::servers.write (guild_id={id})");
		},
		ModelUpdateKind::UserServerSettings { user_id, server_id, user_connections } => {
			MELLOW_MODELS.member_settings.insert((server_id, user_id), UserSettings {
				user_connections
			});
			println!("model::mellow::member_settings.write (guild_id={server_id}) (user_id={user_id})");
		},
		ModelUpdateKind::UserConnection { user_id, connection } => {
			if let Some(mut user) = HAKUMI_MODELS.users.get_mut(&user_id) {
				println!("model::hakumi::users::(id={user_id})::connections.write (id={})", connection.id);
				if let Some(existing) = user.connections.iter_mut().find(|x| x.id == connection.id || (x.sub == connection.sub && x.kind == connection.kind)) {
					*existing = connection;
				} else {
					user.connections.push(connection);
				}
			}
		},
		ModelUpdateKind::UserConnectionOAuthAuthorisation { user_id, connection_id, oauth_authorisation } => {
			if let Some(mut user) = HAKUMI_MODELS.users.get_mut(&user_id) {
				if let Some(connection) = user.connections.iter_mut().find(|x| x.id == connection_id) {
					println!("model::hakumi::users::(id={user_id})::connections::(id={})::oauth_authorisations.write", connection.id);
					if let Some(existing) = connection.oauth_authorisations.iter_mut().find(|x| x.id == oauth_authorisation.id) {
						*existing = oauth_authorisation;
					} else {
						connection.oauth_authorisations.push(oauth_authorisation);
					}
				}
			}
		},
		ModelUpdateKind::VisualScriptingDocument(document) => {
			println!("model::hakumi::vs_documents.write (id={})", document.id);
			HAKUMI_MODELS.vs_documents.insert(document.id, document);
		}
	}

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

	let response: serde_json::Value = database::DATABASE.from("mellow_server_oauth_authorisations")
		.select("server_id")
		.eq("patreon_campaign_id", &payload.data.relationships.campaign.data.id)
		.limit(1)
		.single()
		.await
		.unwrap()
		.value;

	let user_id = &payload.data.relationships.user.data.id;
	let guild_id: Id<GuildMarker> = serde_json::from_value(response.get("server_id").unwrap().clone())
		.map_err(|_| ApiError::GenericInvalidRequest)?;
	if let Some(user_id) = HAKUMI_MODELS.user_by_discord(guild_id, Id::new(user_id.parse().map_err(|_| ApiError::GenericInvalidRequest)?)).await? {
		let user = HAKUMI_MODELS
			.user(user_id)
			.await?;
		let discord_id = Id::new(user.connections.iter().find(|x| matches!(x.kind, ConnectionKind::Discord)).unwrap().id.to_string().parse().map_err(|_| ApiError::GenericInvalidRequest)?);
		sync_single_user(guild_id, user_id, discord_id, Some(ConnectionMetadata {
			patreon_pledges: vec![PatreonPledge {
				tiers: payload.data.relationships.currently_entitled_tiers.data.iter().map(|x| x.id.clone()).collect(),
				active: payload.data.attributes.patron_status.map_or(false, |x| x == "active_patron"),
				user_id: user.id.value,
				campaign_id: payload.data.relationships.campaign.data.id.clone(),
				connection_id: user.server_connections(guild_id).await?.into_iter().find(|x| matches!(x.kind, ConnectionKind::Patreon)).unwrap().id
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
			.map(hex::decode)
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
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY) {
		let mut commands: Vec<Command> = vec![];
		for command in COMMANDS.iter() {
			if command.is_user {
				commands.push(app_command(command, CommandType::User)?);
			}
			if command.is_slash {
				commands.push(app_command(command, CommandType::ChatInput)?);
			}
			if command.is_message {
				commands.push(app_command(command, CommandType::Message)?);
			}
		}

		INTERACTION.set_global_commands(&commands).await.unwrap();
		Ok(HttpResponse::Ok().finish())
	} else { Err(ApiError::InvalidApiKey) }
}