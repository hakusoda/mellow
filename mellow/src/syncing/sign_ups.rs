use std::time::SystemTime;
use tokio::sync::RwLock;

pub struct SignUp {
	pub user_id: String,
	pub guild_id: String,
	pub created_at: SystemTime,
	pub interaction_token: String
}

pub static SIGN_UPS: RwLock<Vec<SignUp>> = RwLock::const_new(vec![]);

pub async fn create_sign_up(user_id: String, guild_id: String, interaction_token: String) {
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