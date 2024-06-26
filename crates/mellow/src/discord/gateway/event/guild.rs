use twilight_model::gateway::payload::incoming::{ GuildCreate, GuildUpdate, GuildDelete };

use crate::{
	model::discord::DISCORD_MODELS,
	Result
};

pub fn guild_create(guild_create: GuildCreate) -> Result<()> {
	if let GuildCreate::Available(guild) = guild_create {
		let guild_id = guild.id;
		for role in &guild.roles {
			DISCORD_MODELS.roles.insert((guild_id, role.id), role.clone().into());
		}
		println!("inserted {} roles", guild.roles.len());

		DISCORD_MODELS.guilds.insert(guild_id, guild.into());
		tracing::info!("model.discord.guild.create (guild_id={guild_id})");
	}
	
	Ok(())
}

pub fn guild_update(guild_update: GuildUpdate) -> Result<()> {
	if let Some(mut guild) = DISCORD_MODELS.guilds.get_mut(&guild_update.id) {
		guild.update(&guild_update);
	}

	Ok(())
}

pub fn guild_delete(guild_delete: GuildDelete) -> Result<()> {
	let guild_id = guild_delete.id;
	tracing::info!("model.discord.guild.delete (guild_id={guild_id})");

	DISCORD_MODELS.guilds.remove(&guild_id);
	Ok(())
}