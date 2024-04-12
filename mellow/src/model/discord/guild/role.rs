use twilight_model::{
	id::{
		marker::RoleMarker,
		Id
	},
	util::ImageHash,
	guild::{ Role, RoleFlags, RoleTags, Permissions },
	gateway::payload::incoming::RoleUpdate
};

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct CachedRole {
    pub color: u32,
	pub hoist: bool,
	pub icon: Option<ImageHash>,
	pub id: Id<RoleMarker>,
	pub managed: bool,
	pub mentionable: bool,
	pub name: String,
	pub permissions: Permissions,
	pub position: i64,
	pub flags: RoleFlags,
	pub tags: Option<RoleTags>,
	pub unicode_emoji: Option<String>
}

impl CachedRole {
	pub fn update(&mut self, role_update: &RoleUpdate) {
		let role = &role_update.role;
		tracing::info_span!("model.discord.role.update", ?role_update.guild_id, ?role.id);
		
		self.color = role.color;
		self.hoist = role.hoist;
		self.icon = role.icon;
		self.managed = role.managed;
		self.mentionable = role.mentionable;
		self.name = role.name.clone();
		self.permissions = role.permissions;
		self.position = role.position;
		self.flags = role.flags;
		self.tags = role.tags.clone();
		self.unicode_emoji = role.unicode_emoji.clone();
	}
}

impl std::hash::Hash for CachedRole {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl Into<CachedRole> for Role {
	fn into(self) -> CachedRole {
		let Role {
			color,
			hoist,
			icon,
			id,
			managed,
			mentionable,
			name,
			permissions,
			position,
			flags,
			tags,
			unicode_emoji,
		} = self;
		CachedRole {
			color,
			hoist,
			icon,
			id,
			managed,
			mentionable,
			name,
			permissions,
			position,
			flags,
			tags,
			unicode_emoji
		}
	}
}