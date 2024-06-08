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

use super::event;
use crate::{
	model::discord::{
		guild::CachedMember,
		DISCORD_MODELS
	},
	Result
};

pub struct Context {
	message_sender: MessageSender,
	pub member_requests: DashMap<u8, oneshot::Sender<()>>,
	pub member_request_index: Mutex<u8>
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
		let event_kind = event.kind();
		tracing::info!("handle_event {event_kind:?}");

		tokio::spawn(async move {
			tracing::debug!("event_kind {event_kind:?} >");

			if let Err(error) =  match event {
				Event::GuildCreate(x) => event::guild::guild_create(*x),
				Event::GuildUpdate(x) => event::guild::guild_update(*x),
				Event::GuildDelete(x) => event::guild::guild_delete(x),
				Event::InteractionCreate(x) => event::interaction::interaction_create(self, *x).await,
				Event::MemberAdd(x) => event::member::member_add(*x).await,
				Event::MemberChunk(x) => event::member::member_chunk(self, x).await,
				Event::MemberUpdate(x) => event::member::member_update(*x).await,
				Event::MemberRemove(x) => event::member::member_remove(x).await,
				Event::MessageCreate(x) => event::message::message_create(*x).await,
				Event::RoleCreate(x) => event::role::role_create(x),
				Event::RoleUpdate(x) => event::role::role_update(x),
				Event::RoleDelete(x) => event::role::role_delete(x),
				_ => Ok(())
			} {
				println!("error occurred in event handler! {error}");
			}

			tracing::debug!("event_kind {event_kind:?} <");
		});
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

			self.member_requests.insert(*index, tx);

			let request = RequestGuildMembers::builder(guild_id)
				.nonce(index.to_string())
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