use twilight_model::{
	id::{
		marker::{ UserMarker, ChannelMarker },
		Id
	},
	util::ImageHash,
	guild::{
		Guild,
		MfaLevel,
		NSFWLevel,
		AfkTimeout,
		Permissions,
		PremiumTier,
		GuildFeature,
		VerificationLevel,
		DefaultMessageNotificationLevel
	},
	gateway::payload::incoming::GuildUpdate
};

pub mod role;
pub use role::CachedRole;

pub mod member;
pub use member::CachedMember;

pub mod onboarding;
pub use onboarding::CachedOnboarding;

#[derive(Debug)]
pub struct CachedGuild {
    pub afk_channel_id: Option<Id<ChannelMarker>>,
    pub afk_timeout: AfkTimeout,
    //pub application_id: Option<Id<ApplicationMarker>>,
    pub banner: Option<ImageHash>,
    pub default_message_notifications: DefaultMessageNotificationLevel,
    pub description: Option<String>,
    //pub discovery_splash: Option<ImageHash>,
    //pub explicit_content_filter: ExplicitContentFilter,
    pub features: Vec<GuildFeature>,
    pub icon: Option<ImageHash>,
    //pub id: Id<GuildMarker>,
    //pub joined_at: Option<Timestamp>,
    //pub large: bool,
    pub max_members: Option<u64>,
    pub max_presences: Option<u64>,
    //pub max_video_channel_users: Option<u64>,
   // pub member_count: Option<u64>,
    pub mfa_level: MfaLevel,
    pub name: String,
    pub nsfw_level: NSFWLevel,
    pub owner_id: Id<UserMarker>,
    pub owner: Option<bool>,
    pub permissions: Option<Permissions>,
    pub preferred_locale: String,
    //pub premium_progress_bar_enabled: bool,
    pub premium_subscription_count: Option<u64>,
    pub premium_tier: PremiumTier,
    //pub public_updates_channel_id: Option<Id<ChannelMarker>>,
    //pub rules_channel_id: Option<Id<ChannelMarker>>,
    //pub safety_alerts_channel_id: Option<Id<ChannelMarker>>,
    pub splash: Option<ImageHash>,
    pub system_channel_id: Option<Id<ChannelMarker>>,
    //pub system_channel_flags: SystemChannelFlags,
    //pub unavailable: bool,
    pub vanity_url_code: Option<String>,
    pub verification_level: VerificationLevel,
    pub widget_channel_id: Option<Id<ChannelMarker>>,
    pub widget_enabled: Option<bool>,
}

impl CachedGuild {
	pub fn update(&mut self, guild_update: &GuildUpdate) {
		tracing::info_span!("model.discord.guild.update", ?guild_update.id);
		self.afk_channel_id = guild_update.afk_channel_id;
        self.afk_timeout = guild_update.afk_timeout;
        self.banner = guild_update.banner;
        self.default_message_notifications = guild_update.default_message_notifications;
        self.description.clone_from(&guild_update.description);
        self.features.clone_from(&guild_update.features);
        self.icon = guild_update.icon;
        self.max_members = guild_update.max_members;
        self.max_presences = Some(guild_update.max_presences.unwrap_or(25000));
        self.mfa_level = guild_update.mfa_level;
        self.name.clone_from(&guild_update.name);
        self.nsfw_level = guild_update.nsfw_level;
        self.owner = guild_update.owner;
        self.owner_id = guild_update.owner_id;
        self.permissions = guild_update.permissions;
        self.preferred_locale.clone_from(&guild_update.preferred_locale);
        self.premium_tier = guild_update.premium_tier;
        self.premium_subscription_count.replace(guild_update.premium_subscription_count.unwrap_or_default());
        self.splash = guild_update.splash;
        self.system_channel_id = guild_update.system_channel_id;
        self.verification_level = guild_update.verification_level;
        self.vanity_url_code.clone_from(&guild_update.vanity_url_code);
        self.widget_channel_id = guild_update.widget_channel_id;
        self.widget_enabled = guild_update.widget_enabled;
	}
}

impl From<Guild> for CachedGuild {
	fn from(value: Guild) -> Self {
		let Guild {
			afk_channel_id,
			afk_timeout,
			//application_id,
			//approximate_member_count,
			//approximate_presence_count,
			banner,
			//channels,
			default_message_notifications,
			description,
			//discovery_splash,
			//emojis,
			//explicit_content_filter,
			features,
			icon,
			//id,
			//joined_at,
			//large,
			max_members,
			max_presences,
			//max_video_channel_users,
			//member_count,
			//members,
			mfa_level,
			name,
			nsfw_level,
			owner_id,
			owner,
			permissions,
			preferred_locale,
			//premium_progress_bar_enabled,
			premium_subscription_count,
			premium_tier,
			//presences,
			//public_updates_channel_id,
			//roles,
			//rules_channel_id,
			//safety_alerts_channel_id,
			splash,
			//stage_instances,
			//stickers,
			//system_channel_flags,
			system_channel_id,
			//threads,
			//unavailable,
			vanity_url_code,
			verification_level,
			//voice_states,
			widget_channel_id,
			widget_enabled,
			..
		} = value;
		Self {
			afk_channel_id,
			afk_timeout,
			//application_id,
			//approximate_member_count,
			//approximate_presence_count,
			banner,
			//channels,
			default_message_notifications,
			description,
			//discovery_splash,
			//emojis,
			//explicit_content_filter,
			features,
			icon,
			//id,
			//joined_at,
			//large,
			max_members,
			max_presences,
			//max_video_channel_users,
			//member_count,
			//members,
			mfa_level,
			name,
			nsfw_level,
			owner_id,
			owner,
			permissions,
			preferred_locale,
			//premium_progress_bar_enabled,
			premium_subscription_count,
			premium_tier,
			//presences,
			//public_updates_channel_id,
			//roles,
			//rules_channel_id,
			//safety_alerts_channel_id,
			splash,
			//stage_instances,
			//stickers,
			//system_channel_flags,
			system_channel_id,
			//threads,
			//unavailable,
			vanity_url_code,
			verification_level,
			//voice_states,
			widget_channel_id,
			widget_enabled
		}
	}
}