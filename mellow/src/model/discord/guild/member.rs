use twilight_model::{
	id::{
		marker::{ RoleMarker, UserMarker },
		Id
	},
	util::{ ImageHash, Timestamp },
	guild::{ Member, MemberFlags },
	gateway::payload::incoming::MemberUpdate
};

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct CachedMember {
    pub avatar: Option<ImageHash>,
	pub communication_disabled_until: Option<Timestamp>,
	pub deaf: Option<bool>,
	pub flags: MemberFlags,
	pub joined_at: Timestamp,
	pub mute: Option<bool>,
	pub nick: Option<String>,
	pub pending: bool,
	pub premium_since: Option<Timestamp>,
	pub roles: Vec<Id<RoleMarker>>,
	pub user_id: Id<UserMarker>
}

impl CachedMember {
	pub fn update(&mut self, member_update: &MemberUpdate) {
		tracing::info_span!("model.discord.member.update", ?member_update.guild_id, ?member_update.user.id);
		self.avatar = member_update.avatar;
		self.communication_disabled_until = member_update.communication_disabled_until;
		self.deaf = member_update.deaf.or(self.deaf);
		self.mute = member_update.mute.or(self.mute);
		self.nick = member_update.nick.clone();
		self.pending = member_update.pending;
		self.premium_since = member_update.premium_since;
		self.roles = member_update.roles.clone();
	}
}

impl std::hash::Hash for CachedMember {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.user_id.hash(state);
	}
}

impl Into<CachedMember> for Member {
	fn into(self) -> CachedMember {
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
		} = self;
		CachedMember {
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