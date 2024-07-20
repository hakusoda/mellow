use dashmap::{
	mapref::one::Ref,
	DashMap
};
use mellow_models::discord::{ guild::{ MemberModel, OnboardingModel, RoleModel }, GuildModel, UserModel };
use twilight_model::id::{
	marker::{ GuildMarker, RoleMarker, UserMarker },
	Id
};	

use crate::Result;

#[derive(Default)]
pub struct DiscordCache {
	pub guilds: DashMap<Id<GuildMarker>, GuildModel>,
	guild_onboardings: DashMap<Id<GuildMarker>, OnboardingModel>,
	pub members: DashMap<(Id<GuildMarker>, Id<UserMarker>), MemberModel>,
	pub roles: DashMap<(Id<GuildMarker>, Id<RoleMarker>), RoleModel>,
	pub users: DashMap<Id<UserMarker>, UserModel>
}

impl DiscordCache {
	pub async fn guild(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, GuildModel>> {
		Ok(match self.guilds.get(&guild_id) {
			Some(model) => model,
			None => {
				let new_model = GuildModel::get(guild_id)
					.await?;
				self.guilds.entry(guild_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn guild_onboarding(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, OnboardingModel>> {
		Ok(match self.guild_onboardings.get(&guild_id) {
			Some(model) => model,
			None => {
				let new_model = OnboardingModel::get(guild_id)
					.await?;
				self.guild_onboardings.entry(guild_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn member(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Ref<'_, (Id<GuildMarker>, Id<UserMarker>), MemberModel>> {
		let key = (guild_id, user_id);
		Ok(match self.members.get(&key) {
			Some(model) => model,
			None => {
				let new_model = MemberModel::get(guild_id, user_id)
					.await?;
				self.members.entry(key)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	#[allow(clippy::type_complexity)]
	pub fn role(&self, guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> Option<Ref<'_, (Id<GuildMarker>, Id<RoleMarker>), RoleModel>> {
		self.roles.get(&(guild_id, role_id))
	}

	pub async fn user(&self, user_id: Id<UserMarker>) -> Result<Ref<'_, Id<UserMarker>, UserModel>> {
		Ok(match self.users.get(&user_id) {
			Some(model) => model,
			None => {
				let new_model = UserModel::get(user_id)
					.await?;
				self.users.entry(user_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}
}