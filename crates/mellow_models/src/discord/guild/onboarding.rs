use mellow_util::DISCORD_CLIENT;
use twilight_model::{
	guild::onboarding::Onboarding,
	id::{ marker::GuildMarker, Id }
};

use crate::Result;

#[derive(Debug)]
pub struct OnboardingModel {
    pub enabled: bool
}

impl OnboardingModel {
	pub async fn get(guild_id: Id<GuildMarker>) -> Result<Self> {
		Ok(DISCORD_CLIENT
			.guild_onboarding(guild_id)
			.await?
			.model()
			.await?
			.into()
		)
	}
}

impl From<Onboarding> for OnboardingModel {
	fn from(value: Onboarding) -> Self {
		Self {
			enabled: value.enabled
		}
	}
}