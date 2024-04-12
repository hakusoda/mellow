use twilight_model::{
	guild::PartialMember,
	application::interaction::application_command::InteractionMember
};

use crate::model::discord::guild::CachedMember;

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

impl Partial<PartialMember> for &CachedMember {
	fn partial(self) -> PartialMember {
		PartialMember {
			avatar: self.avatar,
			communication_disabled_until: self.communication_disabled_until,
			deaf: self.deaf.unwrap_or(false),
			flags: self.flags,
			joined_at: self.joined_at,
			mute: self.mute.unwrap_or(false),
			nick: self.nick.clone(),
			permissions: None,
			premium_since: self.premium_since,
			roles: self.roles.clone(),
			user: None
		}
	}
}