use twilight_model::id;

pub trait QuickId {
	type Id;
	fn id(&self) -> &Self::Id;
}

impl QuickId for twilight_model::guild::PartialMember {
	type Id = id::Id<id::marker::UserMarker>;
	fn id(&self) -> &Self::Id {
		&self.user.as_ref().unwrap().id
	}
}