use std::collections::HashSet;
use dashmap::{
	mapref::one::Ref,
	DashMap
};
use once_cell::sync::Lazy;
use twilight_model::id::{
	marker::{ RoleMarker, UserMarker, GuildMarker },
	Id
};

use user::CachedUser;
use guild::{ CachedRole, CachedGuild, CachedMember, CachedOnboarding };
use crate::{
	discord::CLIENT,
	Result
};

pub mod user;
pub mod guild;

pub static DISCORD_MODELS: Lazy<DiscordModels> = Lazy::new(DiscordModels::default);

#[derive(Debug, Default)]
pub struct DiscordModels {
	pub guilds: DashMap<Id<GuildMarker>, CachedGuild>,
	pub guild_members: DashMap<Id<GuildMarker>, HashSet<Id<UserMarker>>>,
	pub guild_onboardings: DashMap<Id<GuildMarker>, CachedOnboarding>,

	pub roles: DashMap<(Id<GuildMarker>, Id<RoleMarker>), CachedRole>,
	pub members: DashMap<(Id<GuildMarker>, Id<UserMarker>), CachedMember>,

	pub users: DashMap<Id<UserMarker>, CachedUser>
}

impl DiscordModels {
	pub async fn guild(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, CachedGuild>> {
		Ok(if let Some(item) = self.guilds.get(&guild_id) {
			tracing::info!("guilds.read (guild_id={guild_id})");
			item
		} else {
			let new_item = CLIENT.guild(guild_id).await?.model().await?;
			tracing::info!("guilds.write (guild_id={guild_id})");
			
			for member in &new_item.members {
				self.members.insert((guild_id, member.user.id), member.clone().into());
			}
			self.guild_members.insert(guild_id, new_item.members.iter().map(|x| x.user.id).collect());
			self.guilds
				.entry(guild_id)
				.insert(new_item.into())
				.downgrade()
		})
	}

	pub async fn guild_onboarding(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, CachedOnboarding>> {
		Ok(if let Some(item) = self.guild_onboardings.get(&guild_id) {
			tracing::info!("guild_onboarding.read (guild_id={guild_id})");
			item
		} else {
			let new_item = CLIENT.guild_onboarding(guild_id).await?.model().await?;
			tracing::info!("guild_onboarding.write (guild_id={guild_id})");

			self.guild_onboardings
				.entry(guild_id)
				.insert(new_item.into())
				.downgrade()
		})
	}

	#[allow(clippy::type_complexity)]
	pub fn role(&self, guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> Option<Ref<'_, (Id<GuildMarker>, Id<RoleMarker>), CachedRole>> {
		self.roles.get(&(guild_id, role_id))
	}

	pub async fn member(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Ref<'_, (Id<GuildMarker>, Id<UserMarker>), CachedMember>> {
		let key = (guild_id, user_id);
		Ok(if let Some(item) = self.members.get(&key) {
			tracing::info!("members.read (guild_id={guild_id}) (user_id={user_id})");
			item
		} else {
			let new_item = CLIENT.guild_member(guild_id, user_id).await?.model().await?;
			tracing::info!("members.write (guild_id={guild_id}) (user_id={user_id})");

			self.members
				.entry(key)
				.insert(new_item.into())
				.downgrade()
		})
	}

	pub async fn user(&self, user_id: Id<UserMarker>) -> Result<Ref<'_, Id<UserMarker>, CachedUser>> {
		Ok(if let Some(item) = self.users.get(&user_id) {
			tracing::info!("users.read (user_id={user_id})");
			item
		} else {
			let new_item = CLIENT.user(user_id).await?.model().await?;
			tracing::info!("users.write (user_id={user_id})");

			self.users
				.entry(user_id)
				.insert(new_item.into())
				.downgrade()
		})
	}
}