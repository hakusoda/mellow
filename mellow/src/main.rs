#![feature(let_chains, duration_constructors)]
use std::time::{ Duration, SystemTime };
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tokio_stream::StreamExt;
use simple_logger::SimpleLogger;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	application::interaction::Interaction
};

use util::member_into_partial;
use discord::{
	gateway::event_handler::process_element_for_member,
	get_member
};
use visual_scripting::{ Variable, DocumentKind };

mod http;
mod util;
mod cache;
mod error;
mod fetch;
mod traits;
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
	slash_action: Option<fn(Interaction) -> BoxFuture<'static, Result<SlashResponse>>>,
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

	let job_cancel = CancellationToken::new();
	tokio::spawn(spawn_onboarding_job(job_cancel.clone()));
	tokio::spawn(discord::gateway::initialise());
	http::start().await?;

	job_cancel.cancel();

	Ok(())
}

pub static PENDING_VERIFICATION_TIMER: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>, SystemTime)>> = RwLock::const_new(vec![]);

async fn spawn_onboarding_job(stop_signal: CancellationToken) {
	loop {
		if let Ok(mut entries) = PENDING_VERIFICATION_TIMER.try_write() {
			entries.retain(|entry| {
				if entry.2.elapsed().unwrap() >= Duration::from_mins(10) {
					let (guild_id, user_id, _) = entry.clone();
					tokio::spawn(async move {
						let document = database::get_server_event_response_tree(&guild_id, DocumentKind::MemberCompletedOnboardingEvent).await.unwrap();
						if document.is_ready_for_stream(){
							let member = member_into_partial(get_member(&guild_id, &user_id).await.unwrap());
							let (mut stream, mut tracker) = document.into_stream(Variable::create_map([
								("member", Variable::from_partial_member(None, &member, &guild_id))
							], None));
							while let Some((element, variables)) = stream.next().await {
								if process_element_for_member(&element, &variables, &mut tracker).await.unwrap() { break }
							}
							tracker.send_logs(&guild_id).await.unwrap();
						}
					});
					false
				} else { true }
			});
		}

		tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(10)) => {
                continue;
            }

            _ = stop_signal.cancelled() => {
                log::info!("gracefully shutting down onboarding job");
                break;
            }
        };
	}
}

pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;

pub use error::Result;

#[macro_export]
macro_rules! cast {
	($target: expr, $pat: path) => {
		{
			if let $pat(a) = $target {
				Some(a)
			} else {
				None
			}
		}
	};
}