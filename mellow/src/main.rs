use std::net::SocketAddr;
use hyper::{
	server::conn::http1,
	service::service_fn
};
use tokio::net::TcpListener;
use hyper_util::rt::TokioIo;
use simple_logger::SimpleLogger;

use interaction::InteractionPayload;

mod roblox;
mod server;
mod discord;
mod syncing;
mod commands;
mod database;
mod interaction;
mod http_service;

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
	SimpleLogger::new().init().unwrap();

	let address = SocketAddr::from(([127, 0, 0, 1], 8080));
	let listener = TcpListener::bind(address).await?;
	log::info!("now listening for http interactions!");

	loop {
		let (stream, _) = listener.accept().await?;

		let io = TokioIo::new(stream);
		tokio::task::spawn(async move {
			if let Err(err) = http1::Builder::new()
				.serve_connection(io, service_fn(http_service::service))
				.await
			{
				println!("Error serving connection: {:?}", err);
			}
		});
	}
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;