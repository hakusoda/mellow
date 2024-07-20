use dashmap::{ mapref::multiple::RefMulti, DashMap };
use mellow_cache::CACHE;
use mellow_models::discord::guild::MemberModel;
use tokio::sync::{
	Mutex,
	oneshot
};
use twilight_model::{
	id::{
		marker::{ GuildMarker, UserMarker },
		Id
	},
	gateway::payload::outgoing::RequestGuildMembers
};
use twilight_gateway::MessageSender;

use crate::Result;

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

	pub async fn members(&self, guild_id: Id<GuildMarker>, user_ids: Vec<Id<UserMarker>>) -> Result<Vec<RefMulti<'_, (Id<GuildMarker>, Id<UserMarker>), MemberModel>>> {
		let user_ids2: Vec<Id<UserMarker>> = user_ids
			.iter()
			.filter(|user_id| !CACHE.discord.members.contains_key(&(guild_id, **user_id)))
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
		Ok(CACHE
			.discord
			.members
			.iter()
			.filter(|x| x.key().0 == guild_id && user_ids.contains(&x.user_id))
			.collect()
		)
	}
}