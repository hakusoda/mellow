use serde::Deserialize;
use tracing::{ Instrument, info_span };
use once_cell::sync::Lazy;
use postgrest::PostgrestClient;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	cache::CACHES,
	visual_scripting::Document,
	Result
};

pub static DATABASE: Lazy<PostgrestClient> = Lazy::new(|| {
	PostgrestClient::new("https://hakumi.supabase.co/rest/v1")
		.unwrap()
		.with_supabase_key(env!("SUPABASE_API_KEY"))
		.unwrap()
});

#[derive(Clone, Deserialize)]
pub struct ServerCommand {
	pub document: Document,
	#[serde(default)]
	pub is_ephemeral: bool
}

impl ServerCommand {
	pub async fn fetch(guild_id: Id<GuildMarker>, command_name: String) -> Result<Self> {
		let cache_key = (guild_id, command_name);
		Ok(match CACHES.server_commands.get(&cache_key)
			.instrument(info_span!("cache.server_commands.read", ?cache_key))
			.await {
				Some(x) => x,
				None => {
					let command: Self = DATABASE.from("mellow_server_commands")
						.select("document:visual_scripting_documents ( id, name, kind, active, definition ), is_ephemeral")
						.eq("name", &cache_key.1)
						.eq("server_id", guild_id.to_string())
						.limit(1)
						.single()
						.await?
						.value;
					let span = info_span!("cache.server_commands.write", ?cache_key);
					CACHES.server_commands.insert(cache_key, command.clone())
						.instrument(span)
						.await;
	
					command
				}
			}
		)
	}
}

pub async fn server_exists(id: &Id<GuildMarker>) -> Result<bool> {
	Ok(!DATABASE.from("mellow_servers")
		.select::<()>("*")
		.head()
		.eq("id", id)
		.limit(1)
		.await?
		.is_empty()
	)
}