use twilight_model::{
	id::{
		marker::UserMarker,
		Id
	},
	user::{ User, UserFlags },
	util::ImageHash
};

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct CachedUser {
    pub accent_color: Option<u32>,
	pub avatar: Option<ImageHash>,
	pub avatar_decoration: Option<ImageHash>,
	pub banner: Option<ImageHash>,
	pub bot: bool,
	pub discriminator: u16,
	pub flags: Option<UserFlags>,
	pub global_name: Option<String>,
	pub id: Id<UserMarker>,
	pub name: String,
	pub public_flags: Option<UserFlags>,
	pub system: Option<bool>
}

impl CachedUser {
	pub fn display_name(&self) -> &str {
		self.global_name.as_ref().unwrap_or(&self.name)
	}

	pub fn avatar_url(&self) -> Option<String> {
		self.avatar.as_ref().map(|hash| format!("https://cdn.discordapp.com/avatars/{}/{hash}.webp", self.id))
	}
}

impl std::hash::Hash for CachedUser {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl From<User> for CachedUser {
	fn from(value: User) -> Self {
		let User {
			accent_color,
			avatar,
			avatar_decoration,
			banner,
			bot,
			discriminator,
			flags,
			global_name,
			id,
			name,
			public_flags,
			system,
			..
		} = value;
		Self {
			accent_color,
			avatar,
			avatar_decoration,
			banner,
			bot,
			discriminator,
			flags,
			global_name,
			id,
			name,
			public_flags,
			system
		}
	}
}