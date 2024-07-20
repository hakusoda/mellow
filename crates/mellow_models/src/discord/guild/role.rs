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
pub struct RoleModel {
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

impl RoleModel {
	pub fn update(&mut self, role_update: &RoleUpdate) {
		let role = &role_update.role;
		self.color = role.color;
		self.hoist = role.hoist;
		self.icon = role.icon;
		self.managed = role.managed;
		self.mentionable = role.mentionable;
		self.name.clone_from(&role.name);
		self.permissions = role.permissions;
		self.position = role.position;
		self.flags = role.flags;
		self.tags.clone_from(&role.tags);
		self.unicode_emoji.clone_from(&role.unicode_emoji);
	}
}

impl std::hash::Hash for RoleModel {
	fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
		self.id.hash(state);
	}
}

impl From<Role> for RoleModel {
	fn from(value: Role) -> Self {
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
		} = value;
		Self {
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