use twilight_model::{
	guild::PartialMember,
	application::interaction::application_command::InteractionMember
};

pub trait Partial<T> {
	fn partial(self) -> T;
}

impl Partial<PartialMember> for InteractionMember {
	fn partial(self) -> PartialMember {
		PartialMember {
			avatar: self.avatar,
			communication_disabled_until: self.communication_disabled_until,
			deaf: false,
			flags: self.flags,
			joined_at: self.joined_at,
			mute: false,
			nick: self.nick,
			permissions: Some(self.permissions),
			premium_since: self.premium_since,
			roles: self.roles,
			user: None
		}
	}
}