use mellow_cache::CACHE;
use mellow_models::mellow::SignUpModel;
use std::time::SystemTime;
use twilight_model::id::{
	marker::{ UserMarker, GuildMarker },
	Id
};

pub async fn create_sign_up(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>, interaction_token: String) {
	if let Some(mut existing) = CACHE.mellow.sign_ups.get_mut(&user_id) {
		existing.created_at = SystemTime::now();
		existing.guild_id = guild_id;
		existing.interaction_token = interaction_token;
	} else {
		CACHE
			.mellow
			.sign_ups
			.insert(user_id, SignUpModel {
				created_at: SystemTime::now(),
				guild_id,
				interaction_token
			});
	}
}