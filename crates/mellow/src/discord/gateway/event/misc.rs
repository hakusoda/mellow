use mellow_cache::CACHE;
use mellow_models::mellow::ServerModel;
use twilight_model::gateway::payload::incoming::Ready;

use crate::Result;

pub async fn ready(ready: Ready) -> Result<()> {
	let guild_ids: Vec<_> = ready
		.guilds
		.iter()
		.map(|x| x.id)
		.collect();
	for server in ServerModel::get_many(&guild_ids).await? {
		CACHE
			.mellow
			.servers
			.insert(server.id, server);
	}

	Ok(())
}