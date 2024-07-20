use mellow_cache::CACHE;
use mellow_util::DISCORD_CLIENT;
use twilight_model::{
	gateway::payload::incoming::{ GuildCreate, GuildUpdate, GuildDelete },
	guild::Role,
	id::{ marker::GuildMarker, Id }
};

use crate::Result;

fn add_roles_to_cache(guild_id: Id<GuildMarker>, roles: &Vec<Role>) {
	for role in roles {
		CACHE.discord.roles.insert((guild_id, role.id), role.clone().into());
	}
}

pub fn guild_create(guild_create: GuildCreate) -> Result<()> {
	match guild_create {
		GuildCreate::Available(guild) => {
			let guild_id = guild.id;
			add_roles_to_cache(guild_id, &guild.roles);

			CACHE.discord.guilds.insert(guild_id, guild.into());
			tracing::info!("model.discord.guild.create (guild_id={guild_id})");
		},
		GuildCreate::Unavailable(guild) => if ! guild.unavailable {
			tokio::spawn(async move {
				let guild_id = guild.id;
				let guild = DISCORD_CLIENT
					.guild(guild_id)
					.await
					.unwrap()
					.model()
					.await
					.unwrap();
				add_roles_to_cache(guild_id, &guild.roles);

				CACHE.discord.guilds.insert(guild_id, guild.into());
				tracing::info!("model.discord.guild.create (guild_id={guild_id})");
			});
		}
	}
	
	Ok(())
}

pub fn guild_update(guild_update: GuildUpdate) -> Result<()> {
	if let Some(mut guild) = CACHE.discord.guilds.get_mut(&guild_update.id) {
		guild.update(&guild_update);
	}

	Ok(())
}

pub fn guild_delete(guild_delete: GuildDelete) -> Result<()> {
	let guild_id = guild_delete.id;
	tracing::info!("model.discord.guild.delete (guild_id={guild_id})");

	CACHE.discord.guilds.remove(&guild_id);
	Ok(())
}