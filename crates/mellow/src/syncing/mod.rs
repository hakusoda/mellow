use async_recursion::async_recursion;
use serde::{ Serialize, Deserialize };
use std::collections::HashMap;
use twilight_http::request::AuditLogReason;
use twilight_model::id::{
	marker::{ GuildMarker, RoleMarker, UserMarker },
	Id
};
use uuid::Uuid;

use crate::{
	discord::CLIENT,
	model::{
		discord::DISCORD_MODELS,
		hakumi::{
			id::{
				marker::{ UserMarker as HakuUserMarker, ConnectionMarker, SyncActionMarker },
				HakuId
			},
			user::connection::{
				Connection,
				ConnectionKind
			},
			HAKUMI_MODELS
		},
		mellow::{
			server::sync_action::{
				CriteriaItem,
				SyncAction,
				SyncActionKind
			},
			MELLOW_MODELS
		}
	},
	patreon::UserIdentityField,
	roblox::get_user_group_roles,
	visual_scripting::{ Variable, DocumentKind },
	Result
};

pub mod sign_ups;

#[derive(Debug, Serialize)]
pub struct SyncMemberResult {
	#[serde(skip)]
	pub server_id: Id<GuildMarker>,
	pub role_changes: Vec<RoleChange>,
	#[serde(skip)]
	pub member_status: MemberStatus,
	pub profile_changed: bool,
	pub nickname_change: Option<NicknameChange>,
	pub relevant_connections: Vec<Connection>,
	#[serde(skip)]
	pub is_missing_connections: bool
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RoleChange {
	pub kind: RoleChangeKind,
	pub target_id: Id<RoleMarker>,
	pub display_name: String
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum RoleChangeKind {
	Added,
	Removed
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct NicknameChange(pub Option<String>, pub Option<String>);

#[derive(Debug)]
pub struct PatreonPledge {
	pub tiers: Vec<String>,
	pub active: bool,
	pub user_id: Uuid,
	pub campaign_id: String,
	pub connection_id: HakuId<ConnectionMarker>
}

#[derive(Debug)]
pub struct RobloxMembership {
	pub rank: u8,
	pub role: u64,
	pub user_id: String,
	pub group_id: u64
}

#[derive(Debug)]
pub struct ConnectionMetadata {
	pub patreon_pledges: Vec<PatreonPledge>,
	pub roblox_memberships: Vec<RobloxMembership>
}

#[derive(Debug)]
pub enum MemberStatus {
	Ok,
	Banned,
	Kicked
}

impl MemberStatus {
	pub fn removed(&self) -> bool {
		matches!(self, Self::Banned | Self::Kicked)
	}
}

pub async fn get_connection_metadata(guild_id: Id<GuildMarker>, user_ids: &Vec<HakuId<HakuUserMarker>>) -> Result<ConnectionMetadata> {
	let mut patreon_pledges: Vec<PatreonPledge> = vec![];
	let mut roblox_memberships: Vec<RobloxMembership> = vec![];
	let mut group_ids: Vec<u64> = vec![];

	let actions = MELLOW_MODELS
		.server(guild_id)
		.await?
		.actions
		.clone();
	for action in actions {
		for criteria_item in action.criteria.items {
			match criteria_item {
				CriteriaItem::RobloxGroupMembership { group_id } |
				CriteriaItem::RobloxGroupMembershipRole { group_id, .. } |
				CriteriaItem::RobloxGroupMembershipRoleRankInRange { group_id, .. } => {
					if !group_ids.contains(&group_id) {
						group_ids.push(group_id);
					}
				},
				CriteriaItem::PatreonCampaignTierSubscription { .. } => {
					for user_id in user_ids {
						if let Some(connection) = HAKUMI_MODELS.user(*user_id).await?.server_connections(guild_id).await?.into_iter().find(|x| matches!(x.kind, ConnectionKind::Patreon)) {
							let data = crate::patreon::get_user_memberships(&connection.oauth_authorisations[0]).await?;
							if let Some(included) = data.included {
								for membership in included {
									if let UserIdentityField::Member(member) = membership {
										patreon_pledges.push(PatreonPledge {
											tiers: member.relationships.currently_entitled_tiers.data.0.iter().map(|x| x.id.clone()).collect(),
											active: member.attributes.patron_status.is_some_and(|x| x == "active_patron"),
											user_id: user_id.value,
											campaign_id: member.relationships.campaign.data.id,
											connection_id: connection.id
										});
									}
								}
							}
						}
					}
				},
				_ => {}
			}
		}
	}

	if !group_ids.is_empty() {
		//let roblox_ids: Vec<String> = users.iter().flat_map(|x| x.user.connections.iter().filter(|x| matches!(x.connection.kind, ConnectionKind::Roblox)).map(|x| format!("users/{}", x.connection.sub)).collect::<Vec<String>>()).collect();
		//let items = get_group_memberships("-", Some(format!("user in ['{}']", roblox_ids.join("','")))).await;
		let mut ids: Vec<String> = vec![];
		for user_id in user_ids {
			ids.extend(
				HAKUMI_MODELS
				.user(*user_id)
				.await?
				.server_connections(guild_id)
				.await?
				.into_iter()
				.filter(|x| matches!(x.kind, ConnectionKind::Roblox))
				.map(|x| x.sub.clone())
			);
		}
		for id in ids {
			let roles = get_user_group_roles(&id).await?;
			for role in roles {
				roblox_memberships.push(RobloxMembership {
					role: role.role.id,
					rank: role.role.rank,
					user_id: id.clone(),
					group_id: role.group.id
				});
			}
		}
	}

	Ok(ConnectionMetadata {
		patreon_pledges,
		roblox_memberships
	})
}

fn get_role_name(guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> String {
	DISCORD_MODELS.role(guild_id, role_id)
		.map_or_else(|| "Unknown Role".into(), |x| x.name.clone())
}

// async_recursion required due to a cycle error caused by visual scripting
#[async_recursion]
#[tracing::instrument(level = "trace")]
pub async fn sync_single_user(guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, member_id: Id<UserMarker>, connection_metadata: Option<ConnectionMetadata>) -> Result<SyncMemberResult> {
	let metadata = match connection_metadata {
		Some(x) => x,
		None => get_connection_metadata(guild_id, &vec![user_id]).await?
	};
	sync_member(guild_id, Some(user_id), member_id, &metadata).await
}

#[tracing::instrument(level = "trace")]
pub async fn sync_member(guild_id: Id<GuildMarker>, user_id: Option<HakuId<HakuUserMarker>>, member_id: Id<UserMarker>, connection_metadata: &ConnectionMetadata) -> Result<SyncMemberResult> {
	let roles = DISCORD_MODELS
		.member(guild_id, member_id)
		.await?
		.roles
		.clone();
	
	let mut new_roles = roles.clone();
	let mut role_changes: Vec<RoleChange> = vec![];
	let mut member_status = MemberStatus::Ok;
	let mut criteria_cache: HashMap<(HakuId<SyncActionMarker>, usize), bool> = HashMap::new();
	let mut used_connections: Vec<Connection> = vec![];

	let server = MELLOW_MODELS
		.server(guild_id)
		.await?;
	let actions = server.actions.clone();
	let default_nickname = server.default_nickname.clone();
	for action in actions.iter() {
		let met = member_meets_action_criteria(guild_id, user_id, action, &actions, connection_metadata, &mut criteria_cache, &mut used_connections)
			.await?;
		match &action.kind {
			SyncActionKind::AssignRoles { role_ids, can_remove } => {
				if met {
					if !role_ids.iter().all(|x| new_roles.iter().any(|e| e == x)) {
						for role_id in role_ids.iter().filter(|x| !roles.iter().any(|e| &e == x)) {
							new_roles.push(*role_id);
							role_changes.push(RoleChange {
								kind: RoleChangeKind::Added,
								target_id: *role_id,
								display_name: get_role_name(guild_id, *role_id)
							});
						}
					}
				} else if *can_remove {
					let filtered: Vec<Id<RoleMarker>> = new_roles.iter().filter(|x| !role_ids.contains(x)).cloned().collect();
					if !new_roles.iter().all(|x| filtered.contains(x)) {
						for role_id in role_ids {
							if new_roles.contains(role_id) {
								role_changes.push(RoleChange {
									kind: RoleChangeKind::Removed,
									target_id: *role_id,
									display_name: get_role_name(guild_id, *role_id)
								});
							}
						}
						new_roles = filtered;
					}
				}
			},
			SyncActionKind::BanMember(reasoning) => if met {
				// TODO: notify user via direct messages
				member_status = MemberStatus::Banned;
				CLIENT.create_ban(guild_id, member_id)
					.reason(&format!("Met criteria of {} — {}", action.display_name, reasoning.reason.as_ref().unwrap_or(&"No reason".into())))
					.await?;
				break;
			},
			SyncActionKind::KickMember(reasoning) => if met {
				// TODO: notify user via direct messages
				member_status = MemberStatus::Kicked;
				CLIENT.remove_guild_member(guild_id, member_id)
					.reason(&format!("Met criteria of {} — {}", action.display_name, reasoning.reason.as_ref().unwrap_or(&"No reason".into())))
					.await?;
				break;
			},
			SyncActionKind::ControlFlowCancel(_reasoning) => return Ok(SyncMemberResult {
				server_id: guild_id,
				role_changes: vec![],
				member_status,
				profile_changed: false,
				nickname_change: None,
				relevant_connections: vec![],
				is_missing_connections: false
			}),
			SyncActionKind::ExecuteDocument => unimplemented!()
		};
	}

	let target_nickname = match default_nickname {
		Some(t) => if let Some(user_id) = user_id {
			let user = HAKUMI_MODELS.user(user_id)
				.await?;
			let connections = user.server_connections(guild_id)
				.await?;
			match t.as_str() {
				"{roblox_username}" =>
					connections.into_iter().find(|x| matches!(x.kind, ConnectionKind::Roblox)).and_then(|x| x.username.clone()),
				"{roblox_display_name}" =>
					connections.into_iter().find(|x| matches!(x.kind, ConnectionKind::Roblox)).and_then(|x| x.display_name.clone()),
				_ => None
			}
		} else { None },
		None => None
	};

	let guild = DISCORD_MODELS.guild(guild_id).await?;
	let nickname_change = if let Some(target) = &target_nickname {
		let member = DISCORD_MODELS
			.member(guild_id, member_id)
			.await?;
		if member_id != guild.owner_id && (member.nick.is_none() || member.nick.clone().is_some_and(|x| x != *target)) {
			Some(NicknameChange(member.nick.clone(), Some(target.to_string())))
		} else { None }
	} else { None };

	let profile_changed = !member_status.removed() && !role_changes.is_empty() || nickname_change.is_some();
	if profile_changed {
		let mut request = CLIENT.update_guild_member(guild_id, member_id);
		if !role_changes.is_empty() {
			request = request.roles(&new_roles);
		}
		if nickname_change.is_some() {
			request = request.nick(target_nickname.as_deref());
		}
		request.await?;
	}

	// TODO: better.
	let role_changes2 = role_changes.clone();
	tokio::spawn(async move {
		if let Some(document) = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberSynced).await.unwrap() {
			if document.is_ready_for_stream() {
				let member = DISCORD_MODELS
					.member(guild_id, member_id)
					.await
					.unwrap();
				let variables = Variable::create_map([
					("member", Variable::from_member(member.value(), guild_id).await.unwrap()),
					("guild_id", guild_id.to_string().into()),
					("profile_changes", Variable::create_map([
						("roles", Variable::create_map([
							("added", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Added) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into()),
							("removed", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Removed) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into())
						], None))
					], None))
				], None);
				document
					.clone()
					.process(variables)
					.await.unwrap()
					.send_logs(guild_id)
					.await.unwrap();
			}
		}
	});

	let is_missing_connections = if let Some(user_id) = user_id {
		let user = HAKUMI_MODELS
			.user(user_id)
			.await?;
		let connections = user
			.server_connections(guild_id)
			.await?;
		!actions.iter().all(|x| x.criteria.items.iter().all(|e| e.relevant_connection().map_or(true, |x| connections.iter().any(|e| x == e.kind))))
	} else { false };
	Ok(SyncMemberResult {
		server_id: guild_id,
		role_changes,
		member_status,
		profile_changed,
		nickname_change,
		relevant_connections: used_connections,
		is_missing_connections
	})
}

// this needs to move away from recursion
#[async_recursion]
pub async fn member_meets_action_criteria(
	guild_id: Id<GuildMarker>,
	user_id: Option<HakuId<HakuUserMarker>>,
	action: &SyncAction,
	all_actions: &Vec<SyncAction>,
	connection_metadata: &ConnectionMetadata,
	criteria_cache: &mut HashMap<(HakuId<SyncActionMarker>, usize), bool>,
	used_connections: &mut Vec<Connection>
) -> Result<bool> {
	let criteria = &action.criteria;
	let mut total_met = 0;
	let minimum_amount = criteria.quantifier.minimum();
	for (key, item) in criteria.items.iter().enumerate() {
		let cache_key = (action.id, key);
		if criteria_cache.get(&cache_key).is_some_and(|x| *x) || match item {
			CriteriaItem::HakumiUserConnection { connection_kind } => matches!(user_id, Some(user_id) if
				HAKUMI_MODELS
					.user(user_id)
					.await?
					.server_connections(guild_id)
					.await?
					.into_iter()
					.any(|x| &x.kind == connection_kind)
			),
			CriteriaItem::PatreonCampaignTierSubscription { tier_id, campaign_id } => if let Some(user_id) = user_id {
				if let Some(pledge) = connection_metadata.patreon_pledges.iter().find(|x| x.active && x.user_id == user_id.value && x.campaign_id == *campaign_id && x.tiers.contains(tier_id)) {
					if let Some(connection) = HAKUMI_MODELS.user(user_id).await?.server_connections(guild_id).await.unwrap().into_iter().find(|x| x.id == pledge.connection_id) {
						if !used_connections.contains(connection) {
							used_connections.push(connection.clone());
						}
					}
					true
				} else { false }
			} else { false },
			CriteriaItem::RobloxGroupMembership { .. } |
			CriteriaItem::RobloxGroupMembershipRole { .. } |
			CriteriaItem::RobloxGroupMembershipRoleRankInRange { .. } => if let Some(user_id) = user_id {
				let user = HAKUMI_MODELS
					.user(user_id)
					.await?;
				if let Some(connection) = user
					.server_connections(guild_id)
					.await?
					.into_iter()
					.find(|x| matches!(x.kind, ConnectionKind::Roblox))
				{
					if !used_connections.contains(connection) {
						used_connections.push(connection.clone());
					}

					let roblox_id = &connection.sub;
					match item {
						CriteriaItem::RobloxGroupMembership { group_id } =>
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && e.group_id == *group_id),
						CriteriaItem::RobloxGroupMembershipRole { role_id, .. } =>
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && e.role == *role_id),
						CriteriaItem::RobloxGroupMembershipRoleRankInRange { group_id, range_lower, range_upper } => {
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && e.group_id == *group_id && e.rank >= *range_lower && e.rank <= *range_upper)
						},
						_ => false
					}
				} else { false }
			} else { false },
			CriteriaItem::MellowServerSyncingActions { action_ids, quantifier } => {
				let mut total_met = 0;
				let minimum_amount = quantifier.minimum();
				for action_id in action_ids {
					if let Some(other_action) = all_actions.iter().find(|x| &x.id == action_id) {
						if member_meets_action_criteria(guild_id, user_id, other_action, all_actions, connection_metadata, criteria_cache, used_connections).await? {
							total_met += 1;
							if minimum_amount == Some(total_met) {
								break;
							}
						}
					}
				}

				minimum_amount == Some(total_met) || total_met == action_ids.len()
			},
		} {
			criteria_cache.insert(cache_key, true);

			total_met += 1;
			if minimum_amount == Some(total_met) {
				return Ok(true);
			}
		} else {
			criteria_cache.insert(cache_key, false);
		}
	}
	Ok(minimum_amount.is_none() && total_met == criteria.items.len())
}