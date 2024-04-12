use std::time::Duration;
use moka::future::{ Cache, CacheBuilder };
use once_cell::sync::Lazy;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	patreon::{ Campaign2, UserIdentity },
	database::ServerCommand,
	visual_scripting::{ Document, DocumentKind }
};

pub static CACHES: Lazy<Caches> = Lazy::new(|| Caches {
	event_responses: CacheBuilder::new(32)
		.time_to_live(Duration::from_hours(1))
		.build(),

	patreon_campaigns: CacheBuilder::new(64)
		.time_to_live(Duration::from_mins(30))
		.build(),
	patreon_user_identities: CacheBuilder::new(64)
		.time_to_live(Duration::from_mins(5))
		.build(),

	server_commands: CacheBuilder::new(64)
		.time_to_live(Duration::from_mins(10))
		.build()
});

pub struct Caches {
	pub event_responses: Cache<(Id<GuildMarker>, DocumentKind), Document>,

	pub patreon_campaigns: Cache<String, Campaign2>,
	pub patreon_user_identities: Cache<String, UserIdentity>,

	pub server_commands: Cache<(Id<GuildMarker>, String), ServerCommand>
}