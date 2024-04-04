use twilight_model::guild::{
	Member,
	PartialMember
};

pub fn unwrap_string_or_array(value: &serde_json::Value) -> Option<&str> {
	value.as_array().map_or_else(|| value.as_str(), |x| x.get(0).and_then(|x| x.as_str()))
}

pub fn member_into_partial(member: Member) -> PartialMember {
	PartialMember {
		avatar: member.avatar,
		communication_disabled_until: member.communication_disabled_until,
		deaf: member.deaf,
		flags: member.flags,
		joined_at: member.joined_at,
		mute: member.mute,
		nick: member.nick,
		permissions: None,
		premium_since: member.premium_since,
		roles: member.roles,
		user: Some(member.user)
	}
}