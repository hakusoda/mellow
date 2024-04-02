use std::time::Duration;
use moka::future::{ Cache, CacheBuilder };
use once_cell::sync::Lazy;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	patreon::UserIdentity,
	discord::{ Guild, GuildOnboarding },
	visual_scripting::{ Document, DocumentKind }
};

pub static CACHES: Lazy<Caches> = Lazy::new(|| Caches {
	discord_guilds: CacheBuilder::new(32)
		.time_to_live(Duration::from_hours(1))
		.build(),
	discord_guild_onboarding: CacheBuilder::new(32)
		.time_to_live(Duration::from_hours(1))
		.build(),

	event_responses: CacheBuilder::new(32)
		.time_to_live(Duration::from_hours(1))
		.build(),
	patreon_user_identities: CacheBuilder::new(64)
		.time_to_live(Duration::from_mins(5))
		.build()
});

pub struct Caches {
	pub discord_guilds: Cache<String, Guild>,
	pub discord_guild_onboarding: Cache<String, GuildOnboarding>,

	pub event_responses: Cache<(Id<GuildMarker>, DocumentKind), Document>,
	pub patreon_user_identities: Cache<String, UserIdentity>
}