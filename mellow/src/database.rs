use serde::Deserialize;
use tracing::{ Instrument, info_span };
use once_cell::sync::Lazy;
use postgrest::Postgrest;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	cache::CACHES,
	visual_scripting::Document,
	Result
};

pub const DATABASE: Lazy<Postgrest> = Lazy::new(|| {
	let key = env!("SUPABASE_API_KEY");
	Postgrest::new("https://hakumi.supabase.co/rest/v1")
		.insert_header("apikey", key)
		.insert_header("authorization", format!("Bearer {}", key))
});

#[derive(Clone, Deserialize)]
pub struct ServerCommand {
	pub document: Document
}

impl ServerCommand {
	pub async fn fetch(guild_id: &Id<GuildMarker>, command_name: String) -> Result<Self> {
		let cache_key = (guild_id.clone(), command_name);
		Ok(match CACHES.server_commands.get(&cache_key)
			.instrument(info_span!("cache.server_commands.read", ?cache_key))
			.await {
				Some(x) => x,
				None => {
					let command: Self = simd_json::from_slice(&mut DATABASE.from("mellow_server_commands")
						.select("document:visual_scripting_documents(id,name,kind,active,definition)")
						.eq("name", &cache_key.1)
						.eq("server_id", guild_id.to_string())
						.limit(1)
						.single()
						.execute()
						.await?
						.bytes()
						.await?
						.to_vec()
					)?;
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
	// this isn't an ideal method, but this rust library is way too limited, especially when compared to postgrest-js...
	Ok(DATABASE.from("mellow_servers")
		.select("id")
		.eq("id", id.to_string())
		.limit(1)
		.single()
		.execute()
		.await?
		.status()
		.is_success()
	)
}