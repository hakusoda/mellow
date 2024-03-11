use std::collections::HashMap;
use simple_logger::SimpleLogger;
use twilight_model::gateway::{
	payload::outgoing::update_presence::UpdatePresencePayload,
	presence::{ Status, Activity, ActivityType }
};
use twilight_gateway::{ Event, Shard, Intents, ShardId };

use server::event::start_event_response;
use database::get_server_event_response_tree;
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
		let config = twilight_gateway::Config::builder(env!("DISCORD_TOKEN").to_string(), Intents::GUILD_MEMBERS)
			.presence(UpdatePresencePayload::new(vec![Activity {
				id: None,
				url: None,
				name: "burgers".into(),
				kind: ActivityType::Custom,
				emoji: None,
				flags: None,
				party: None,
				state: Some("now here's the syncer".into()),
				assets: None,
				buttons: vec![],
				details: None,
				secrets: None,
				instance: None,
				created_at: None,
				timestamps: None,
				application_id: None
			}.into()], false, None, Status::Online).unwrap())
			.build();
		let mut shard = Shard::with_config(ShardId::ONE, config);
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
					let response_tree = get_server_event_response_tree(&server_id, "member_join").await.unwrap();
					if !response_tree.is_empty() {
						if let Some(user) = database::get_users_by_discord(vec![user_id.clone()], &server_id).await.into_iter().next() {
							let member = discord::get_member(&server_id, &user_id).await.unwrap();
							start_event_response(&response_tree, &HashMap::from([
								("globals".into(), serde_json::json!({
									"member": {
										"id": member.id(),
										"username": member.user.username,
										"display_name": member.display_name()
									}
								}))
							]), &server_id, Some(&user), Some(&member)).await;
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