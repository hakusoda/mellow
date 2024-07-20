use actix_web::{
	Responder, HttpRequest, HttpResponse,
	get, web, post
};
use hmac::{ Mac, Hmac };
use mellow_cache::CACHE;
use mellow_models::{
	hakumi::{
		user::connection::ConnectionModel,
		DocumentModel
	},
	mellow::server::{ ServerModel, UserSettingsModel }
};
use mellow_util::{
	hakuid::{
		marker::{ ConnectionMarker, DocumentMarker, UserMarker as HakuUserMarker },
		HakuId
	},
	DISCORD_INTERACTION_CLIENT
};
use serde::Deserialize;
use sha2::Sha256;
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
	commands::{
		syncing::sync_with_token,
		COMMANDS
	},
	server::{
		action_log::{ ActionLog, DataChange },
		logging::{ ServerLog, send_logs }
	},
	syncing::{
		ConnectionMetadata, PatreonPledge, SyncingInitiator, SyncMemberResult,
		sync_single_user
	},
	util::user_server_connections,
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
		)
		.service(
			web::scope("internal")
				.service(internal_model_event)	
		);
}

#[get("/")]
async fn index() -> impl Responder {
    HttpResponse::Ok().body(format!("hello from mellow v{CARGO_PKG_VERSION}!\nhttps://github.com/hakusoda/mellow"))
}

#[derive(Deserialize)]
struct SyncMemberPayload {
	webhook_token: Option<String>
}

#[post("/server/{server_id}/member/{member_id}/sync")]
async fn sync_member(request: HttpRequest, body: web::Json<SyncMemberPayload>, path: web::Path<(u64, u64)>) -> ApiResult<web::Json<SyncMemberResult>> {
	// TODO: make this... easier on the eyes.
	if request.headers().get("x-api-key").map_or(false, |x| x.to_str().unwrap() == API_KEY) {
		let (guild_id, member_id) = path.into_inner();
		let guild_id: Id<GuildMarker> = Id::new(guild_id);
		let member_id: Id<UserMarker> = Id::new(member_id);
		if let Some(user_id) = CACHE.hakumi.user_by_discord(guild_id, member_id).await? {
			return Ok(web::Json(if let Some(token) = &body.webhook_token {
				sync_with_token(guild_id, user_id, member_id, token, false, None).await?
			} else {
				sync_single_user(guild_id, user_id, member_id, SyncingInitiator::Automatic, None).await?
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

	send_logs(payload.server_id, vec![ServerLog::ActionLog(payload)])
		.await?;

	Ok(HttpResponse::Ok().finish())
}

#[derive(Debug, Deserialize)]
struct ModelEvent {
	actionee_id: Option<HakuId<HakuUserMarker>>,
	kind: ModelEventKind,
	model: ModelKind
}

#[derive(Debug, Deserialize)]
enum ModelEventKind {
	Created,
	Updated,
	Deleted
}

#[derive(Debug, Deserialize)]
enum ModelKind {
	Server(Id<GuildMarker>),
	UserConnection(HakuId<HakuUserMarker>, HakuId<ConnectionMarker>),
	UserSettings(Id<GuildMarker>, HakuId<HakuUserMarker>),
	VisualScriptingDocument(Option<Id<GuildMarker>>, HakuId<DocumentMarker>)
}

#[post("/model_event")]
async fn internal_model_event(_request: HttpRequest, payload: web::Json<ModelEvent>) -> ApiResult<HttpResponse> {
	let model_update = payload.into_inner();
	match model_update.model {
		ModelKind::Server(guild_id) => match model_update.kind {
			ModelEventKind::Created => (),
			ModelEventKind::Updated => {
				let new_model = ServerModel::get(guild_id)
					.await?
					.unwrap();
				let new_default_nickname = new_model.default_nickname.clone();
				let new_logging_types = new_model.logging_types;
				let new_logging_channel_id = new_model.logging_channel_id;
				if let Some(old_server) = CACHE.mellow.servers.insert(guild_id, new_model) {
					let mut logging_data_changes: Vec<DataChange> = vec![];
					if old_server.default_nickname != new_default_nickname {
						if old_server.default_nickname.is_none() {
							logging_data_changes.push(DataChange::created("Default Nickname", new_default_nickname.unwrap())?);
						} else if new_default_nickname.is_none() {
							logging_data_changes.push(DataChange::deleted("Default Nickname", old_server.default_nickname.unwrap())?);
						} else {
							logging_data_changes.push(DataChange::updated("Default Nickname", old_server.default_nickname.unwrap(), new_default_nickname.unwrap())?);
						}
					}
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
						if let Err(err) = send_logs(guild_id, vec![
							ServerLog::ActionLog(ActionLog {
								kind: "mellow.server.discord_logging.updated".into(),
								author: model_update.actionee_id,
								server_id: guild_id,
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
			},
			ModelEventKind::Deleted => {
				CACHE
					.mellow
					.servers
					.remove(&guild_id);
			}
		},
		ModelKind::UserConnection(user_id, connection_id) => match model_update.kind {
			ModelEventKind::Created => if let Some(connection_ids) = CACHE.hakumi.user_connections.get_mut(&user_id) {
				connection_ids.insert(connection_id);
			},
			ModelEventKind::Updated => if CACHE.hakumi.connections.contains_key(&connection_id) {
				let new_model = ConnectionModel::get(connection_id)
					.await?
					.unwrap();
				CACHE
					.hakumi
					.connections
					.insert(connection_id, new_model);
			},
			ModelEventKind::Deleted => {
				for connection_ids in CACHE.hakumi.user_connections.iter_mut() {
					connection_ids.remove(&connection_id);
				}
				CACHE
					.hakumi
					.connections
					.remove(&connection_id);
			}
		},
		ModelKind::UserSettings(guild_id, user_id) => {
			let model_key = (guild_id, user_id);
			match model_update.kind {
				ModelEventKind::Created => (),
				ModelEventKind::Updated => if CACHE.mellow.user_settings.contains_key(&model_key) {
					let new_model = UserSettingsModel::get(guild_id, user_id)
						.await?;
					CACHE
						.mellow
						.user_settings
						.insert(model_key, new_model);
				},
				ModelEventKind::Deleted => {
					CACHE
						.mellow
						.user_settings
						.remove(&model_key);
				}
			}

			tokio::spawn(async move {
				let user_connections = CACHE
					.hakumi
					.user_connections(&[user_id])
					.await
					.unwrap();
				let connections = CACHE
					.hakumi
					.connections(&user_connections)
					.await
					.unwrap();
				if let Some(connection) = connections.into_iter().find(|x| x.is_discord()) {
					let member_id = Id::new(connection.sub.parse().unwrap());
					if let Some((_,sign_up)) = CACHE.mellow.sign_ups.remove(&member_id) {
						sync_with_token(guild_id, user_id, member_id, &sign_up.interaction_token, true, None)
							.await
							.unwrap();
					} else {
						let result = sync_single_user(guild_id, user_id, member_id, SyncingInitiator::Automatic, None)
							.await
							.unwrap();
						if let Some(result_log) = result.create_log() {
							send_logs(guild_id, vec![result_log])
								.await
								.unwrap();
						}
					}
				}
			});
		},
		ModelKind::VisualScriptingDocument(guild_id, document_id) => match model_update.kind {
			ModelEventKind::Created => if
				let Some(guild_id) = guild_id &&
				let Some(document_ids) = CACHE.mellow.server_visual_scripting_documents.get(&guild_id)
			{
				document_ids.insert(document_id);
			},
			ModelEventKind::Updated => if CACHE.hakumi.visual_scripting_documents.contains_key(&document_id) {
				let new_model = DocumentModel::get(document_id)
					.await?
					.unwrap();
				CACHE
					.hakumi
					.visual_scripting_documents
					.insert(document_id, new_model);
			},
			ModelEventKind::Deleted => {
				for document_ids in CACHE.mellow.server_visual_scripting_documents.iter_mut() {
					document_ids.remove(&document_id);
				}
				CACHE
					.hakumi
					.visual_scripting_documents
					.remove(&document_id);
			}
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
	relationships: PayloadRelationships
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

#[post("/server/{server_id}/webhook/patreon")]
async fn patreon_webhook(payload: web::Payload, path: web::Path<Id<GuildMarker>>) -> ApiResult<HttpResponse> {
	let mut body = payload.to_bytes().await.unwrap().to_vec();
	let payload: PayloadData<WebhookPayload> = simd_json::from_slice(&mut body)
		.map_err(|_| ApiError::GenericInvalidRequest)?;

	let user_id = &payload.data.relationships.user.data.id;
	let guild_id = path.into_inner();
	if let Some(user_id) = CACHE.hakumi.user_by_discord(guild_id, Id::new(user_id.parse().map_err(|_| ApiError::GenericInvalidRequest)?)).await? {
		let connection_ids = CACHE
			.hakumi
			.user_connections(&[user_id])
			.await?;
		let connections = CACHE
			.hakumi
			.connections(&connection_ids)
			.await?;
		let discord_id = Id::new(connections.into_iter().find(|x| x.is_discord()).unwrap().id.to_string().parse().map_err(|_| ApiError::GenericInvalidRequest)?);
		sync_single_user(guild_id, user_id, discord_id, SyncingInitiator::Automatic, Some(ConnectionMetadata {
			issues: Vec::new(),
			patreon_pledges: vec![PatreonPledge {
				campaign_id: payload.data.relationships.campaign.data.id.clone(),
				connection_id: user_server_connections(guild_id, user_id)
					.await?
					.into_iter()
					.find(|x| x.is_patreon())
					.unwrap()
					.id,
				tiers: payload.data.relationships.currently_entitled_tiers.data.iter().map(|x| x.id.clone()).collect(),
				user_id: user_id.value
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

		DISCORD_INTERACTION_CLIENT
			.set_global_commands(&commands)
			.await
			.unwrap();
		Ok(HttpResponse::Ok().finish())
	} else { Err(ApiError::InvalidApiKey) }
}