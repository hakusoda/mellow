use tokio::sync::{
	Mutex,
	oneshot
};
use dashmap::{ mapref::multiple::RefMulti, DashMap };
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	gateway::payload::outgoing::RequestGuildMembers
};
use twilight_gateway::{ Event, MessageSender };

use super::event_handler;
use crate::{
	model::{
		discord::{
			guild::CachedMember,
			DISCORD_MODELS
		},
		mellow::MELLOW_MODELS
	},
	server::logging::ServerLog,
	interaction::handle_interaction,
	Result
};

pub struct Context {
	message_sender: MessageSender,
	member_requests: DashMap<u8, oneshot::Sender<()>>,
	member_request_index: Mutex<u8>
}

impl Context {
	pub fn new(message_sender: MessageSender) -> Self {
		Self {
			message_sender,
			member_requests: DashMap::new(),
			member_request_index: Mutex::new(0)
		}
	}

	pub async fn handle_event(self: crate::Context, event: Event) -> Result<()> {
		tracing::info!("handle_event kind: {:?}", event.kind());
		match event {
			Event::InteractionCreate(event_data) => {
				handle_interaction(self, event_data.0).await?;
			},
			Event::MemberAdd(event_data) => {
				DISCORD_MODELS.members.insert((event_data.guild_id, event_data.user.id), event_data.member.clone().into());
				tracing::info!("model.discord.member.create (guild_id={}) (user_id={})", event_data.guild_id, event_data.user.id);
				if let Err(error) = event_handler::member_add(&event_data).await {
					MELLOW_MODELS.server(event_data.guild_id)
						.await?
						.send_logs(vec![ServerLog::VisualScriptingProcessorError {
							error: error.to_string(),
							document_name: "New Member Event".into()
						}])
						.await?;
				}
			},
			Event::MemberUpdate(event_data) => {
				for mut member in DISCORD_MODELS.members.iter_mut() {
					if event_data.guild_id == member.key().0 && event_data.user.id == member.user_id {
						member.update(&event_data);
						break;
					}
				}
				event_handler::member_update(&event_data).await?;
				tracing::info!("done with member update");
			},
			Event::MemberRemove(event_data) => {
				DISCORD_MODELS.members.remove(&(event_data.guild_id, event_data.user.id));
				tracing::info!("model.discord.member.delete (guild_id={}) (user_id={})", event_data.guild_id, event_data.user.id);
			},
			Event::MessageCreate(event_data) => {
				if !event_data.author.bot {
					event_handler::message_create(&event_data).await?;
				}
			},
			Event::RoleCreate(event_data) => {
				tracing::info!("model.discord.role.create (guild_id={}) (role_id={})", event_data.guild_id, event_data.role.id);
				DISCORD_MODELS.roles.insert((event_data.guild_id, event_data.role.id), event_data.role.into());
			},
			Event::RoleUpdate(event_data) => {
				for mut role in DISCORD_MODELS.roles.iter_mut() {
					if event_data.guild_id == role.key().0 && event_data.role.id == role.id {
						role.update(&event_data);
						break;
					}
				}
			},
			Event::RoleDelete(event_data) => {
				DISCORD_MODELS.roles.remove(&(event_data.guild_id, event_data.role_id));
				tracing::info!("model.discord.role.delete (guild_id={}) (role_id={})", event_data.guild_id, event_data.role_id);
			},
			Event::GuildCreate(event_data) => {
				tracing::info!("model.discord.guild.create (guild_id={})", event_data.id);
				DISCORD_MODELS.guilds.insert(event_data.id, event_data.0.into());
			},
			Event::GuildUpdate(event_data) => {
				for mut guild in DISCORD_MODELS.guilds.iter_mut() {
					if event_data.id == guild.id {
						guild.update(&event_data);
						break;
					}
				}
			},
			Event::GuildDelete(event_data) => {
				DISCORD_MODELS.guilds.remove(&event_data.id);
				tracing::info!("model.discord.guild.delete (guild_id={})", event_data.id);
			},
			Event::MemberChunk(event_data) => {
				for member in event_data.members {
					DISCORD_MODELS.members.insert((event_data.guild_id, member.user.id), member.into());
				}
				if event_data.chunk_index == event_data.chunk_count - 1 && let Some(nonce) = event_data.nonce.and_then(|x| x.parse().ok()) {
					if let Some(value) = self.member_requests.remove(&nonce) {
						tracing::info!("done with member request");
						value.1.send(()).unwrap();
						*self.member_request_index.lock().await -= 1;
					}
				}
			},
			_ => ()
		};
		Ok(())
	}

	pub async fn members(&self, guild_id: Id<GuildMarker>, user_ids: Vec<Id<UserMarker>>) -> Result<Vec<RefMulti<'_, (Id<GuildMarker>, Id<UserMarker>), CachedMember>>> {
		let user_ids2: Vec<Id<UserMarker>> = user_ids
			.iter()
			.filter(|user_id| !DISCORD_MODELS.members.contains_key(&(guild_id, **user_id)))
			.copied()
			.collect();

		if !user_ids2.is_empty() {
			let (tx, rx) = oneshot::channel();
			let mut index = self.member_request_index.lock().await;
			*index += 1;

			let request_id = index.clone();
			self.member_requests.insert(request_id, tx);

			let request = RequestGuildMembers::builder(guild_id)
				.nonce(request_id.to_string())
				.user_ids(user_ids2)?;
			self.message_sender.command(&request)?;

			rx.await?;
		}
		Ok(DISCORD_MODELS.members
			.iter()
			.filter(|x| x.key().0 == guild_id && user_ids.contains(&x.user_id))
			.collect()
		)
	}
}