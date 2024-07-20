use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::SyncActionMarker,
		HakuId
	},
	PG_POOL
};
use serde::Deserialize;
use std::pin::Pin;
use twilight_model::id::{
	marker::RoleMarker,
	Id
};

use crate::{
	hakumi::user::connection::ConnectionKind,
	Result
};

#[derive(Clone, Debug)]
pub struct SyncActionModel {
	pub id: HakuId<SyncActionMarker>,
	pub kind: SyncActionKind,
	pub criteria: Criteria,
	pub display_name: String
}

impl SyncActionModel {
	pub async fn get(sync_action_id: HakuId<SyncActionMarker>) -> Result<Option<Self>> {
		Self::get_many(&[sync_action_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(sync_action_ids: &[HakuId<SyncActionMarker>]) -> Result<Vec<Self>> {
		if sync_action_ids.is_empty() {
			return Ok(vec![]);
		}

		let sync_action_ids: Vec<_> = sync_action_ids
			.iter()
			.map(|x| x.value)
			.collect();
		Ok(sqlx::query!(
			"
			SELECT id, kind, criteria, action_data, display_name
			FROM mellow_server_sync_actions
			WHERE id = ANY($1)
			",
			&sync_action_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, record| {
				acc.push(Self {
					id: record.id.into(),
					kind: serde_json::from_value(serde_json::json!({
						"kind": record.kind,
						"action_data": record.action_data
					})).unwrap(),
					criteria: serde_json::from_value(record.criteria).unwrap(),
					display_name: record.display_name
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
pub struct Reasoning {
	#[serde(default)]
	pub reason: Option<String>/*,
	pub user_facing_details: Option<String>*/
}

#[derive(Clone, Debug, Deserialize)]
pub struct Criteria {
	pub items: Vec<CriteriaItem>,
	pub quantifier: Quantifier
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Clone, Debug, Deserialize)]
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
		group_id: u64,
		role_id: u64
	},
	#[serde(rename = "roblox.group.membership.role.rank.in_range")]
	RobloxGroupMembershipRoleRankInRange {
		group_id: u64,
		range_lower: u8,
		range_upper: u8
	},

	#[serde(rename = "patreon.campaign.tier_subscription")]
	PatreonCampaignTierSubscription {
		campaign_id: String,
		tier_id: String
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