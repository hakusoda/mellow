use uuid::Uuid;
use serde::{ Serialize, Deserialize };
use serde_repr::{ Serialize_repr, Deserialize_repr };

use crate::model::hakumi::user::connection::ConnectionKind;

#[derive(Debug, Serialize, Deserialize)]
pub struct SyncAction {
	pub id: Uuid,
	pub name: String,
	#[serde(rename = "type")]
	pub kind: SyncActionKind,
	pub metadata: serde_json::Value,
	pub requirements: Vec<Requirement>,
	pub requirements_type: RequirementsKind
}

#[derive(Clone, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum SyncActionKind {
	GiveRoles,
	BanFromServer,
	KickFromServer,
	CancelSync
}

#[derive(Eq, Hash, Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Requirement {
	pub id: String,
	#[serde(rename = "type")]
	pub kind: RequirementKind,
	pub data: Vec<String>
}

impl Requirement {
	pub fn relevant_connection(&self) -> Option<ConnectionKind> {
		match self.kind {
			RequirementKind::RobloxHaveConnection |
			RequirementKind::RobloxHaveGroupRole |
			RequirementKind::RobloxHaveGroupRankInRange |
			RequirementKind::RobloxInGroup |
			RequirementKind::RobloxBeFriendsWith |
			RequirementKind::RobloxHaveAsset |
			RequirementKind::RobloxHaveBadge |
			RequirementKind::RobloxHavePass => Some(ConnectionKind::Roblox),
			RequirementKind::GitHubInOrganisation => Some(ConnectionKind::GitHub),
			RequirementKind::PatreonHaveCampaignTier => Some(ConnectionKind::Patreon),
			_ => None
		}
	}
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum RequirementKind {
	RobloxHaveConnection,
	RobloxHaveGroupRole,
	RobloxHaveGroupRankInRange,
	RobloxInGroup,
	RobloxBeFriendsWith,
	MeetOtherAction,
	HAKUMIInTeam,
	SteamInGroup,
	RobloxHaveAsset,
	RobloxHaveBadge,
	RobloxHavePass,
	GitHubInOrganisation,
	PatreonHaveCampaignTier
}

#[derive(Clone, Debug, Serialize_repr, Deserialize_repr)]
#[repr(u8)]
pub enum RequirementsKind {
	MeetAll,
	MeetOne
}