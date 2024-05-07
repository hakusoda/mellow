use serde::Deserialize;
use twilight_model::id::{
	marker::RoleMarker,
	Id
};

use crate::model::hakumi::{
	id::{
		marker::SyncActionMarker,
		HakuId
	},
	user::connection::ConnectionKind
};

#[derive(Debug, Deserialize)]
pub struct SyncAction {
	pub id: HakuId<SyncActionMarker>,
	#[serde(flatten)]
	pub kind: SyncActionKind,
	pub criteria: Criteria,
	pub display_name: String
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", content = "action_data")]
pub enum SyncActionKind {
	#[serde(rename = "discord.member.assign_roles")]
	AssignRoles {
		role_ids: Vec<Id<RoleMarker>>,
		can_remove: bool
	},
	#[serde(rename = "discord.member.ban")]
	BanMember(Reasoning),
	#[serde(rename = "discord.member.kick")]
	KickMember(Reasoning),
	#[serde(rename = "visual_scripting.execute_document")]
	ExecuteDocument,
	#[serde(rename = "control_flow.cancel")]
	ControlFlowCancel(Reasoning)
}

#[derive(Debug, Deserialize)]
pub struct Reasoning {
	pub reason: Option<String>/*,
	pub user_facing_details: Option<String>*/
}

#[derive(Debug, Deserialize)]
pub struct Criteria {
	pub items: Vec<CriteriaItem>,
	pub quantifier: Quantifier
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Quantifier {
	All,
	AtLeast {
		value: u8
	}
}

impl Quantifier {
	pub fn minimum(&self) -> Option<usize> {
		match self {
			Quantifier::AtLeast { value } => Some(*value as usize),
			_ => None
		}
	}
}

#[derive(Debug, Deserialize)]
#[serde(tag = "kind")]
pub enum CriteriaItem {
	#[serde(rename = "hakumi.user.connection")]
	HakumiUserConnection {
		connection_kind: ConnectionKind
	},

	#[serde(rename = "mellow.server.syncing.actions")]
	MellowServerSyncingActions {
		action_ids: Vec<HakuId<SyncActionMarker>>,
		quantifier: Quantifier
	},

	#[serde(rename = "roblox.group.membership")]
	RobloxGroupMembership {
		group_id: u64
	},
	#[serde(rename = "roblox.group.membership.role")]
	RobloxGroupMembershipRole {
		role_id: u64,
		group_id: u64
	},
	#[serde(rename = "roblox.group.membership.role.rank.in_range")]
	RobloxGroupMembershipRoleRankInRange {
		group_id: u64,
		range_lower: u8,
		range_upper: u8
	},

	#[serde(rename = "patreon.campaign.tier_subscription")]
	PatreonCampaignTierSubscription {
		tier_id: String,
		campaign_id: String
	}
}

impl CriteriaItem {
	pub fn relevant_connection(&self) -> Option<ConnectionKind> {
		match self {
			Self::HakumiUserConnection { connection_kind } => Some(connection_kind.clone()),
			Self::PatreonCampaignTierSubscription { .. } => Some(ConnectionKind::Patreon),
			Self::RobloxGroupMembership { .. } |
			Self::RobloxGroupMembershipRole { .. } |
			Self::RobloxGroupMembershipRoleRankInRange { .. } => Some(ConnectionKind::Roblox),
			_ => None
		}
	}
}