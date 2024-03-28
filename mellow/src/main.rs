use simple_logger::SimpleLogger;

use interaction::InteractionPayload;

mod http;
mod util;
mod cache;
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

	tokio::spawn(discord::gateway::initialise());
	http::start().await
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub use error::Result;