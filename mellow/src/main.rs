#![feature(async_closure)]
use simple_logger::SimpleLogger;
use twilight_model::gateway::{
	payload::outgoing::update_presence::UpdatePresencePayload,
	presence::{ Status, Activity, ActivityType }
};
use twilight_gateway::{ Event, Shard, Intents, ShardId };

use server::ServerLog;
use interaction::InteractionPayload;

mod http;
mod error;
mod fetch;
mod roblox;
mod server;
mod discord;
mod syncing;
mod patreon;
mod commands;
mod database;
mod interaction;
mod visual_scripting;

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
					content: Some(format!("{error}\n{}", error.context))
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
					tokio::spawn(async move {
						if let Err(error) = discord::gateway::event_handler::member_add(&data).await {
							database::get_server(data.guild_id.to_string()).await.unwrap().send_logs(vec![ServerLog::VisualScriptingProcessorError {
								error: error.to_string(),
								document_name: "New Member Event".into()
							}]).await.unwrap();
						}
					});
				},
				_ => ()
			}
		}
	});

	http::start().await
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub use error::Result;

#[macro_export]
macro_rules! cast {
	($target: expr, $pat: path) => {
		{
			if let $pat(a) = $target {
				a
			} else {
				panic!("mismatch variant when cast to {}", stringify!($pat));
			}
		}
	};
}