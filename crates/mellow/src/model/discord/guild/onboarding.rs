use twilight_model::guild::onboarding::Onboarding;

#[derive(Debug)]
pub struct CachedOnboarding {
    pub enabled: bool
}

impl From<Onboarding> for CachedOnboarding {
	fn from(value: Onboarding) -> Self {
		Self {
			enabled: value.enabled
		}
	}
}