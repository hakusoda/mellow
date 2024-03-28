use std::time::Duration;
use moka::future::{ Cache, CacheBuilder };
use once_cell::sync::Lazy;

use crate::{
	discord::Guild,
	patreon::UserIdentity,
	visual_scripting::{ Document, DocumentKind }
};

pub static CACHES: Lazy<Caches> = Lazy::new(|| Caches {
	discord_guilds: CacheBuilder::new(32)
		.time_to_live(Duration::from_secs(3600))
		.build(),
	event_responses: CacheBuilder::new(32)
		.time_to_live(Duration::from_secs(3600))
		.build(),
	patreon_user_identities: CacheBuilder::new(64)
		.time_to_live(Duration::from_secs(300))
		.build()
});

pub struct Caches {
	pub discord_guilds: Cache<String, Guild>,
	pub event_responses: Cache<(String, DocumentKind), Document>,
	pub patreon_user_identities: Cache<String, UserIdentity>
}