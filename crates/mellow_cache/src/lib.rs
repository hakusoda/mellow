#![feature(let_chains)]
use once_cell::sync::Lazy;

use discord::DiscordCache;
use hakumi::HakumiCache;
use mellow::MellowCache;
use patreon::PatreonCache;

pub mod discord;
pub mod error;
pub mod hakumi;
pub mod mellow;
pub mod patreon;

pub use error::{ Error, Result };

#[derive(Default)]
pub struct Cache {
	pub discord: DiscordCache,
	pub hakumi: HakumiCache,
	pub mellow: MellowCache,
	pub patreon: PatreonCache
}

pub static CACHE: Lazy<Cache> = Lazy::new(Cache::default);