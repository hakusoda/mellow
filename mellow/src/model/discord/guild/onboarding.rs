use twilight_model::guild::onboarding::Onboarding;

#[derive(Debug)]
pub struct CachedOnboarding {
    pub enabled: bool
}

impl Into<CachedOnboarding> for Onboarding {
	fn into(self) -> CachedOnboarding {
		let Onboarding {
			//default_channel_ids,
			enabled,
			//guild_id,
			//prompts,,
			..
		} = self;
		CachedOnboarding {
			enabled
		}
	}
}