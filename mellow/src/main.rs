#![feature(let_chains, try_blocks, duration_constructors)]
use std::{
	sync::Arc,
	time::{ Duration, SystemTime }
};
use tokio::sync::RwLock;
use tracing::{ Level, info };
use mimalloc::MiMalloc;
use tokio_util::sync::CancellationToken;
use tracing_log::LogTracer;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	guild::Permissions,
	channel::message::MessageFlags,
	application::interaction::Interaction
};
use tracing_subscriber::FmtSubscriber;

use error::ErrorKind;
use model::{
	discord::DISCORD_MODELS,
	mellow::MELLOW_MODELS
};
use traits::Partial;
use discord::INTERACTION;
use visual_scripting::{ Variable, DocumentKind };

mod http;
mod util;
mod cache;
mod error;
mod fetch;
mod model;
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

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

pub type Context = Arc<discord::gateway::Context>;

pub struct Command {
	name: String,
	no_dm: bool,
	handler: fn(Context, Interaction) -> BoxFuture<'static, Result<CommandResponse>>,
	is_user: bool,
	is_slash: bool,
	is_message: bool,
	description: Option<String>,
	default_member_permissions: Option<String>
}

impl Command {
	pub fn default_member_permissions(&self) -> Result<Option<Permissions>> {
		Ok(if let Some(permissions) = self.default_member_permissions.as_ref() {
			Some(Permissions::from_bits_truncate(permissions.parse()?))
		} else { None })
	}
}

pub enum CommandKind {
	Slash,
	User,
	Message
}

pub enum CommandResponse {
	Message {
		flags: Option<MessageFlags>,
		content: Option<String>
	},
	Defer
}

impl CommandResponse {
	pub fn defer(interaction_token: impl Into<String>, callback: BoxFuture<'static, Result<()>>) -> Self {
		let interaction_token = interaction_token.into();
		tokio::spawn(async move {
			if let Err(error) = callback.await {
				tracing::error!("error during interaction: {}", error);
				let (text, problem) = match &error.kind {
					ErrorKind::TwilightHttpError(error) => (" while communicating with discord...", error.to_string()),
					_ => (", not sure what exactly though!", error.to_string())
				};
				INTERACTION.update_response(&interaction_token)
					.content(Some(&format!("<:niko_look_left:1227198516590411826> something unexpected happened{text}\n```diff\n- {problem}\n--- {}```", error.context))).unwrap()
					.await.unwrap();
			}
		});
		Self::Defer
	}

	pub fn ephemeral(content: impl Into<String>) -> Self {
		Self::Message {
			flags: Some(MessageFlags::EPHEMERAL),
			content: Some(content.into())
		}
	}
}

#[tokio::main]
async fn main() -> std::io::Result<()> {
	let subscriber = FmtSubscriber::builder()
        .with_max_level(Level::INFO)
        .finish();

    tracing::subscriber::set_global_default(subscriber)
        .expect("setting default subscriber failed");

	LogTracer::init().unwrap();

	info!("starting mellow v{}", env!("CARGO_PKG_VERSION"));

	let job_cancel = CancellationToken::new();
	tokio::spawn(spawn_onboarding_job(job_cancel.clone()));

	http::initialise().await?;
	discord::gateway::initialise().await;

	job_cancel.cancel();

	info!("shutting down mellow...goodbye!");
	Ok(())
}

pub static PENDING_VERIFICATION_TIMER: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>, SystemTime)>> = RwLock::const_new(vec![]);

async fn spawn_onboarding_job(stop_signal: CancellationToken) {
	loop {
		if let Ok(mut entries) = PENDING_VERIFICATION_TIMER.try_write() {
			entries.retain(|entry| {
				// this is ten seconds under ten minutes to compensate for the job's sleeping time
				if entry.2.elapsed().unwrap() >= Duration::from_secs(590) {
					info!("removing {entry:?} from PENDING_VERIFICATION_TIMER");
					let (guild_id, user_id, _) = entry.clone();
					tokio::spawn(async move {
						let document = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await.unwrap();
						if document.is_ready_for_stream(){
							let member = DISCORD_MODELS.member(guild_id, user_id).await.unwrap().partial();
							let variables = Variable::create_map([
								("member", Variable::from_partial_member(None, &member, &guild_id))
							], None);
							document
								.process(variables)
								.await.unwrap()
								.send_logs(guild_id)
								.await.unwrap();
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
                info!("gracefully shutting down onboarding job");
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