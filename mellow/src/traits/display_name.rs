pub trait DisplayName {
	fn display_name(&self) -> &str;
}

impl DisplayName for twilight_model::guild::PartialMember {
	fn display_name(&self) -> &str {
		self.nick.as_ref().map(|x| x.as_str()).unwrap_or_else(|| {
			self.user.as_ref().map_or("MISSINGNO", |user| {
				user.global_name.as_ref().map(|x| x.as_str()).unwrap_or(user.name.as_str())
			})
		})
	}
}