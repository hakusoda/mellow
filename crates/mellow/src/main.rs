#![feature(let_chains, try_blocks, duration_constructors)]
use mellow_cache::CACHE;
use mellow_models::hakumi::visual_scripting::{ DocumentKind, Variable };
use mellow_util::DISCORD_INTERACTION_CLIENT;
use std::{
	sync::Arc,
	time::{ Duration, SystemTime }
};
use tokio::sync::RwLock;
use tokio_util::sync::CancellationToken;
use tracing::{ Level, info };
use tracing_log::LogTracer;
use tracing_subscriber::FmtSubscriber;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	guild::Permissions,
	channel::message::MessageFlags
};

use error::Error;
use interaction::Interaction;
use visual_scripting::{ process_document, variable_from_member };

mod commands;
mod discord;
mod error;
mod http;
mod interaction;
mod util;
mod roblox;
mod server;
mod syncing;
mod visual_scripting;

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
				let (text, problem) = match &error {
					Error::TwilightHttp(error) => (" while communicating with discord...", error.to_string()),
					_ => (", not sure what exactly though!", error.to_string())
				};
				DISCORD_INTERACTION_CLIENT
					.update_response(&interaction_token)
					.content(Some(&format!("<:niko_look_left:1227198516590411826> something unexpected happened{text}\n```diff\n- {problem}```")))
					.await
					.unwrap();
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

#[tokio::main(flavor = "multi_thread")]
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

#[allow(clippy::type_complexity)]
pub static PENDING_VERIFICATION_TIMER: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>, SystemTime)>> = RwLock::const_new(vec![]);

async fn spawn_onboarding_job(stop_signal: CancellationToken) {
	loop {
		if let Ok(mut entries) = PENDING_VERIFICATION_TIMER.try_write() {
			entries.retain(|entry| {
				// this is ten seconds under ten minutes to compensate for the job's sleeping time
				if entry.2.elapsed().unwrap() >= Duration::from_secs(590) {
					info!("removing {entry:?} from PENDING_VERIFICATION_TIMER");
					let (guild_id, user_id, _) = *entry;
					tokio::spawn(async move {
						if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await.unwrap() {
							if let Some(document) = document.clone_if_ready() {
								let variables = Variable::create_map([
									("member", variable_from_member(guild_id, user_id).await.unwrap())
								], None);
								process_document(document, variables)
									.await
									.send_logs(guild_id)
									.await.unwrap();
							}
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