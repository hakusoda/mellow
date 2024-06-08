use twilight_model::gateway::payload::incoming::{ RoleCreate, RoleUpdate, RoleDelete };

use crate::{
	model::discord::DISCORD_MODELS,
	Result
};

pub fn role_create(role_create: RoleCreate) -> Result<()> {
	let role_id = role_create.role.id;
	let guild_id = role_create.guild_id;
	tracing::info!("model.discord.role.create (guild_id={guild_id}) (role_id={role_id})");
	
	DISCORD_MODELS.roles.insert((guild_id, role_id), role_create.role.into());
	Ok(())
}

pub fn role_update(role_update: RoleUpdate) -> Result<()> {
	if let Some(mut role) = DISCORD_MODELS.roles.get_mut(&(role_update.guild_id, role_update.role.id)) {
		role.update(&role_update);
	}

	Ok(())
}

pub fn role_delete(role_delete: RoleDelete) -> Result<()> {
	let role_id = role_delete.role_id;
	let guild_id = role_delete.guild_id;
	tracing::info!("model.discord.role.delete (guild_id={guild_id}) (role_id={role_id})");

	DISCORD_MODELS.roles.remove(&(guild_id, role_id));
	Ok(())
}