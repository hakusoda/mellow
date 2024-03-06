use simple_logger::SimpleLogger;
use twilight_gateway::{ Event, Shard, Intents, ShardId };

use server::{ ServerLog, ProfileSyncKind };
use interaction::InteractionPayload;

mod http;
mod error;
mod fetch;
mod roblox;
mod server;
mod discord;
mod syncing;
mod commands;
mod database;
mod interaction;

pub struct Command {
	name: &'static str,
	no_dm: bool,
	description: Option<String>,
	slash_action: Option<fn(InteractionPayload) -> BoxFuture<'static, Result<SlashResponse>>>,
	default_member_permissions: Option<String>
}

pub enum SlashResponse {
	Message {
		flags: Option<u8>,
		content: Option<String>
	},
	DeferMessage
}

impl SlashResponse {
	pub fn defer(interaction_token: impl Into<String>, callback: BoxFuture<'static, Result<()>>) -> SlashResponse {
		let interaction_token = interaction_token.into();
		tokio::spawn(async move {
			if let Err(error) = callback.await {
				discord::edit_original_response(interaction_token, interaction::InteractionResponseData::ChannelMessageWithSource {
					flags: None,
					embeds: None,
					content: Some(error.in_current_span().to_string())
				}).await.unwrap();
			}
		});
		SlashResponse::DeferMessage
	}
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	SimpleLogger::new()
		.with_level(log::LevelFilter::Info)
		.env()
		.init()
		.unwrap();

	tokio::spawn(async {
		let mut shard = Shard::new(ShardId::ONE, env!("DISCORD_TOKEN").to_string(), Intents::GUILD_MEMBERS);
		loop {
			let event = match shard.next_event().await {
				Ok(event) => event,
				Err(source) => {
					tracing::warn!(?source, "error receiving event");
					if source.is_fatal() {
						break;
					}
	
					continue;
				}
			};

			match event {
				Event::MemberAdd(data) => {
					let user_id = data.user.id.to_string();
					let server_id = data.guild_id.to_string();
					if let Some(user) = database::get_users_by_discord(vec![user_id.clone()], &server_id).await.into_iter().next() {
						let member = discord::get_member(&server_id, &user_id).await.unwrap();
						let result = syncing::sync_single_user(&user, &member, server_id).await.unwrap();
						if result.profile_changed {
							result.server.send_logs(vec![ServerLog::ServerProfileSync {
								kind: ProfileSyncKind::NewMember,
								member,
								forced_by: None,
								role_changes: result.role_changes.clone(),
								nickname_change: result.nickname_change.clone(),
								relevant_connections: result.relevant_connections.clone()
							}]).await.unwrap();
						}
					}
				},
				_ => ()
			}
		}
	});

	http::start().await
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub use error::Result;
use tracing_error::InstrumentError;