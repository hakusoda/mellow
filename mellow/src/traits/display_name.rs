pub trait DisplayName {
	fn display_name(&self) -> &str;
}

impl DisplayName for twilight_model::guild::PartialMember {
	fn display_name(&self) -> &str {
		self.nick.as_ref().unwrap_or_else(|| {
			let user = self.user.as_ref().unwrap();
			user.global_name.as_ref().unwrap_or(&user.name)
		})
	}
}