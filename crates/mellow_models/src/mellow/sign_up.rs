use std::time::SystemTime;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

pub struct SignUpModel {
	pub created_at: SystemTime,
	pub guild_id: Id<GuildMarker>,
	pub interaction_token: String
}