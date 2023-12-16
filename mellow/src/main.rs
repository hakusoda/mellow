use simple_logger::SimpleLogger;

use interaction::InteractionPayload;

mod http;
mod roblox;
mod server;
mod discord;
mod syncing;
mod commands;
mod database;
mod interaction;

pub struct Command {
	name: &'static str,
	slash_action: Option<fn(InteractionPayload) -> BoxFuture<'static, SlashResponse>>
}

pub enum SlashResponse {
	Message {
		flags: Option<u8>,
		content: Option<String>
	},
	DeferMessage
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
	SimpleLogger::new()
		.with_level(log::LevelFilter::Info)
		.env()
		.init()
		.unwrap();

	http::start().await
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;