use super::QuickId;

pub trait AvatarUrl {
	fn avatar_url(&self) -> Option<String>;
}

impl AvatarUrl for twilight_model::guild::PartialMember {
	fn avatar_url(&self) -> Option<String> {
		self.avatar.or(self.user.as_ref().and_then(|x| x.avatar)).as_ref().map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", self.id()))
	}
}