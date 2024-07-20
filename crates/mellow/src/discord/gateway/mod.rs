use std::sync::Arc;
use twilight_model::gateway::{
	payload::outgoing::update_presence::UpdatePresencePayload,
	presence::{ Status, Activity, ActivityType }
};
use twilight_gateway::{ Shard, Intents, ShardId, StreamExt, ConfigBuilder, EventTypeFlags };

pub use context::Context;

mod context;
pub mod event;

pub async fn initialise() {
	tracing::info!("initialising discord gateway");

	let config = ConfigBuilder::new(
		env!("DISCORD_BOT_TOKEN").to_string(),
			Intents::GUILDS | Intents::GUILD_MEMBERS | Intents::GUILD_MESSAGES |
			Intents::MESSAGE_CONTENT
	)
		.presence(UpdatePresencePayload::new(vec![Activity {
			id: None,
			url: None,
			name: "burgers".into(),
			kind: ActivityType::Custom,
			emoji: None,
			flags: None,
			party: None,
			state: Some(std::env::var("DISCORD_STATUS_TEXT").unwrap_or("now here's the syncer".into())),
			assets: None,
			buttons: vec![],
			details: None,
			secrets: None,
			instance: None,
			created_at: None,
			timestamps: None,
			application_id: None
		}], false, None, Status::Online).unwrap())
		.build();
	let mut shard = Shard::with_config(ShardId::ONE, config);
	let context = Arc::new(Context::new(shard.sender()));
	
	while let Some(item) = shard.next_event(EventTypeFlags::all()).await {
		let Ok(event) = item else {
			tracing::warn!(source = ?item.unwrap_err(), "error receiving event");
			continue;
		};

		event::handle_event(&context, event);
	}
}