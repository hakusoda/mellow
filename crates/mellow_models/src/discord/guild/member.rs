use mellow_util::DISCORD_CLIENT;
use twilight_model::{
	id::{
		marker::{ GuildMarker, RoleMarker, UserMarker },
		Id
	},
	util::{ ImageHash, Timestamp },
	guild::{ Member, MemberFlags },
	gateway::payload::incoming::MemberUpdate
};

use crate::Result;

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct MemberModel {
    pub avatar: Option<ImageHash>,
	pub communication_disabled_until: Option<Timestamp>,
	pub deaf: Option<bool>,
	pub flags: MemberFlags,
	pub joined_at: Option<Timestamp>,
	pub mute: Option<bool>,
	pub nick: Option<String>,
	pub pending: bool,
	pub premium_since: Option<Timestamp>,
	pub roles: Vec<Id<RoleMarker>>,
	pub user_id: Id<UserMarker>
}

impl MemberModel {
	pub async fn get(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Self> {
		Ok(DISCORD_CLIENT
			.guild_member(guild_id, user_id)
			.await?
			.model()
			.await?
			.into()
		)
	}

	pub fn update(&mut self, member_update: &MemberUpdate) {
		self.avatar = member_update.avatar;
		self.communication_disabled_until = member_update.communication_disabled_until;
		self.deaf = member_update.deaf.or(self.deaf);
		self.mute = member_update.mute.or(self.mute);
		self.nick.clone_from(&member_update.nick);
		self.pending = member_update.pending;
		self.premium_since = member_update.premium_since;
		self.roles.clone_from(&member_update.roles);
	}
}

impl std::hash::Hash for MemberModel {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.user_id.hash(state);
	}
}

impl From<Member> for MemberModel {
	fn from(value: Member) -> Self {
		let Member {
			avatar,
			communication_disabled_until,
			deaf,
			flags,
			joined_at,
			mute,
			nick,
			pending,
			premium_since,
			roles,
			user
		} = value;
		Self {
			avatar,
			communication_disabled_until,
			deaf: Some(deaf),
			flags,
			joined_at,
			mute: Some(mute),
			nick,
			pending,
			premium_since,
			roles,
			user_id: user.id
		}
	}
}