use std::time::SystemTime;
use tokio::sync::RwLock;
use twilight_model::id::{
	marker::{ UserMarker, GuildMarker },
	Id
};

pub struct SignUp {
	pub user_id: Id<UserMarker>,
	pub guild_id: Id<GuildMarker>,
	pub created_at: SystemTime,
	pub interaction_token: String
}

pub static SIGN_UPS: RwLock<Vec<SignUp>> = RwLock::const_new(vec![]);

pub async fn create_sign_up(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>, interaction_token: String) {
	let mut items = SIGN_UPS.write().await;
	if let Some(existing) = items.iter_mut().find(|x| x.user_id == user_id) {
		existing.guild_id = guild_id;
		existing.created_at = SystemTime::now();
		existing.interaction_token = interaction_token;
	} else {
		items.push(SignUp {
			user_id,
			guild_id,
			created_at: SystemTime::now(),
			interaction_token
		});
	}
}