use async_recursion::async_recursion;
use mellow_cache::CACHE;
use mellow_models::{
	hakumi::{
		user::connection::ConnectionKind,
		visual_scripting::{ DocumentKind, Variable }
	},
	mellow::server::sync_action::{
		CriteriaItem,
		SyncActionKind,
		SyncActionModel
	},
	patreon::user_identity::UserIdentityField
};
use mellow_util::{
	hakuid::{
		marker::{ ConnectionMarker, DocumentMarker, SyncActionMarker, UserMarker as HakuUserMarker },
		HakuId
	},
	DISCORD_CLIENT,
	PG_POOL
};
use rand::{ distributions::Alphanumeric, Rng };
use serde::{ Serialize, Deserialize };
use std::{
	collections::HashMap,
	pin::Pin
};
use twilight_http::request::AuditLogReason;
use twilight_model::id::{
	marker::{ GuildMarker, RoleMarker, UserMarker },
	Id
};
use uuid::Uuid;

use crate::{
	roblox::get_user_group_roles,
	server::logging::{ ProfileSyncKind, ServerLog },
	util::user_server_connections,
	visual_scripting::{ process_document, variable_from_member },
	Error, Result
};

pub mod sign_ups;

#[derive(Debug, Serialize)]
pub struct SyncMemberResult {
	#[serde(skip)]
	pub initiator: SyncingInitiator,
	#[serde(skip)]
	pub issues: Vec<SyncingIssue>,
	pub role_changes: Vec<RoleChange>,
	#[serde(skip)]
	pub member_status: MemberStatus,
	pub profile_changed: bool,
	pub nickname_change: Option<NicknameChange>,
	pub relevant_connections: Vec<HakuId<ConnectionMarker>>,
	pub user_id: Id<UserMarker>
}

impl SyncMemberResult {
	pub fn create_log(&self) -> Option<ServerLog> {
		if self.profile_changed || self.member_status.removed() {
			Some(ServerLog::ServerProfileSync {
				kind: match self.member_status {
					MemberStatus::Ok => ProfileSyncKind::Default,
					MemberStatus::Banned => ProfileSyncKind::Banned,
					MemberStatus::Kicked => ProfileSyncKind::Kicked
				},
				initiator: self.initiator.clone(),
				user_id: self.user_id,
				role_changes: self.role_changes.clone(),
				nickname_change: self.nickname_change.clone(),
				relevant_connections: self.relevant_connections.clone()
			})
		} else { None }
	}
}

#[derive(Clone, Debug)]
pub enum SyncingInitiator {
	Automatic,
	ForcedBy(Id<UserMarker>),
	Manual,
	VisualScriptingDocument(HakuId<DocumentMarker>)
}

#[derive(Clone, Debug)]
pub enum SyncingIssue {
	MissingConnections,
	MissingOAuthAuthorisation(ConnectionKind)
}

impl SyncingIssue {
	pub async fn format_many(items: &[Self], guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, website_token: &str) -> Result<String> {
		let mut strings = Vec::with_capacity(items.len());
		for item in items {
			strings.push(item.display(guild_id, user_id, website_token).await?);
		}

		Ok(strings.join("\n"))
	}

	pub async fn display(&self, guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, website_token: &str) -> Result<String> {
		Ok(match self {
			Self::MissingConnections =>
				format!("You haven't given this server access to all connections yet, fix that [here](<https://hakumi.cafe/mellow/server/{guild_id}/user_settings?mt={website_token}>)!"),
			Self::MissingOAuthAuthorisation(connection_kind) => {
				let token: String = rand::thread_rng()
					.sample_iter(Alphanumeric)
					.take(24)
					.map(char::from)
					.collect();
				sqlx::query!(
					"
					INSERT INTO mellow_connection_requests (server_id, user_id, token)
					VALUES ($1, $2, $3)
					ON CONFLICT (user_id)
					DO UPDATE SET token = $3
					",
					guild_id.get() as i64,
					user_id.value,
					&token
				)
					.execute(&*Pin::static_ref(&PG_POOL).await)
					.await?;

				format!("Your {connection_kind:?} connection was invalidated, please [reconnect it](<https://www.patreon.com/oauth2/authorize?client_id=BaKp_8PIeBxx0cfJoEEaVxVQMxD3c_IUFS_qCSu5gNFnXLL5c4Qw4YMPtgMJG-n9&redirect_uri=https%3A%2F%2Fapi-new.hakumi.cafe%2Fv1%2Fconnection_callback%2F4&scope=identity%20identity.memberships&response_type=code&state=m1-{token}>).")
			}
		})
	}
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
	pub campaign_id: String,
	pub connection_id: HakuId<ConnectionMarker>,
	pub tiers: Vec<String>,
	pub user_id: Uuid,
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
	pub issues: Vec<SyncingIssue>,
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
	let mut issues: Vec<SyncingIssue> = Vec::new();
	let mut patreon_pledges: Vec<PatreonPledge> = Vec::new();
	let mut roblox_memberships: Vec<RobloxMembership> = Vec::new();
	let mut group_ids: Vec<u64> = Vec::new();

	let action_ids = CACHE
		.mellow
		.server_sync_actions(guild_id)
		.await?;

	// TODO: no cloning!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
	let actions: Vec<_> = CACHE
		.mellow
		.sync_actions(&action_ids)
		.await?
		.into_iter()
		.map(|x| x.clone())
		.collect();
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
						let connections = user_server_connections(guild_id, *user_id)
							.await?;
						if let Some(connection) = connections.into_iter().find(|x| x.is_patreon()) {
							let connection_id = connection.id;
							drop(connection);
							
							let user_identity = CACHE
								.patreon
								.user_identity(connection_id)
								.await?;
							if let Some(user_identity) = user_identity {
								if let Some(included) = &user_identity.included {
									for membership in included {
										if let UserIdentityField::Member(member) = membership {
											patreon_pledges.push(PatreonPledge {
												campaign_id: member.relationships.campaign.data.id.clone(),
												connection_id,
												tiers: member.relationships.currently_entitled_tiers.data.0.iter().map(|x| x.id.clone()).collect(),
												user_id: user_id.value
											});
										}
									}
								}
							} else if !issues.iter().any(|x| matches!(x, SyncingIssue::MissingOAuthAuthorisation(ConnectionKind::Patreon))) {
								issues.push(SyncingIssue::MissingOAuthAuthorisation(ConnectionKind::Patreon));
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
			let connections = user_server_connections(guild_id, *user_id)
				.await?;
			ids.extend(
				connections
					.into_iter()
					.filter(|x| x.is_roblox())
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
		issues,
		patreon_pledges,
		roblox_memberships
	})
}

fn get_role_name(guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> String {
	CACHE
		.discord
		.role(guild_id, role_id)
		.map_or_else(|| "Unknown Role".into(), |x| x.name.clone())
}

// async_recursion required due to a cycle error caused by visual scripting
#[async_recursion]
#[tracing::instrument(level = "trace")]
pub async fn sync_single_user(guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>, member_id: Id<UserMarker>, initiator: SyncingInitiator, connection_metadata: Option<ConnectionMetadata>) -> Result<SyncMemberResult> {
	let metadata = match connection_metadata {
		Some(x) => x,
		None => get_connection_metadata(guild_id, &vec![user_id]).await?
	};
	sync_member(guild_id, Some(user_id), member_id, initiator, &metadata).await
}

#[tracing::instrument(level = "trace")]
pub async fn sync_member(guild_id: Id<GuildMarker>, user_id: Option<HakuId<HakuUserMarker>>, member_id: Id<UserMarker>, initiator: SyncingInitiator, connection_metadata: &ConnectionMetadata) -> Result<SyncMemberResult> {
	let roles = CACHE
		.discord
		.member(guild_id, member_id)
		.await?
		.roles
		.clone();
	
	let mut issues = connection_metadata.issues.clone();
	let mut new_roles = roles.clone();
	let mut role_changes: Vec<RoleChange> = vec![];
	let mut member_status = MemberStatus::Ok;
	let mut criteria_cache: HashMap<(HakuId<SyncActionMarker>, usize), bool> = HashMap::new();
	let mut used_connections: Vec<HakuId<ConnectionMarker>> = vec![];

	let server = CACHE
		.mellow
		.server(guild_id)
		.ok_or(Error::ServerNotFound)?;
	let default_nickname = server.default_nickname.clone();

	let action_ids = CACHE
		.mellow
		.server_sync_actions(guild_id)
		.await?;

	// TODO: no cloning!!!!!!!!!!!!!!!!!!!!!!!!!!!!!
	let actions: Vec<_> = CACHE
		.mellow
		.sync_actions(&action_ids)
		.await?
		.into_iter()
		.map(|x| x.clone())
		.collect();
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
				DISCORD_CLIENT
					.create_ban(guild_id, member_id)
					.reason(&format!("Met criteria of {} — {}", action.display_name, reasoning.reason.as_ref().unwrap_or(&"No reason".into())))
					.await?;
				break;
			},
			SyncActionKind::KickMember(reasoning) => if met {
				// TODO: notify user via direct messages
				member_status = MemberStatus::Kicked;
				DISCORD_CLIENT
					.remove_guild_member(guild_id, member_id)
					.reason(&format!("Met criteria of {} — {}", action.display_name, reasoning.reason.as_ref().unwrap_or(&"No reason".into())))
					.await?;
				break;
			},
			SyncActionKind::ControlFlowCancel(_reasoning) => return Ok(SyncMemberResult {
				initiator,
				issues,
				role_changes: vec![],
				member_status,
				profile_changed: false,
				nickname_change: None,
				relevant_connections: vec![],
				user_id: member_id
			}),
			SyncActionKind::ExecuteDocument => unimplemented!()
		};
	}

	let target_nickname = match default_nickname {
		Some(t) => if let Some(user_id) = user_id {
			let connections = user_server_connections(guild_id, user_id)
				.await?;
			match t.as_str() {
				"{roblox_username}" =>
					connections.into_iter().find(|x| x.is_roblox()).and_then(|x| x.username.clone()),
				"{roblox_display_name}" =>
					connections.into_iter().find(|x| x.is_roblox()).and_then(|x| x.display_name.clone()),
				_ => None
			}
		} else { None },
		None => None
	};

	let nickname_change = if let Some(target) = &target_nickname {
		let guild_owner_id = CACHE
			.discord
			.guild(guild_id)
			.await?
			.owner_id;
		let member = CACHE
			.discord
			.member(guild_id, member_id)
			.await?;
		if member_id != guild_owner_id && (member.nick.is_none() || member.nick.as_ref().is_some_and(|x| x != target)) {
			Some(NicknameChange(member.nick.clone(), Some(target.to_string())))
		} else { None }
	} else { None };

	let profile_changed = !member_status.removed() && (!role_changes.is_empty() || nickname_change.is_some());
	if profile_changed {
		let mut request = DISCORD_CLIENT.update_guild_member(guild_id, member_id);
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
		if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MemberSynced).await.unwrap() {
			if let Some(document) = document.clone_if_ready() {
				let variables = Variable::create_map([
					("member", variable_from_member(guild_id, member_id).await.unwrap()),
					("guild_id", guild_id.to_string().into()),
					("profile_changes", Variable::create_map([
						("roles", Variable::create_map([
							("added", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Added) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into()),
							("removed", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Removed) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into())
						], None))
					], None))
				], None);
				process_document(document, variables)
					.await
					.send_logs(guild_id)
					.await
					.unwrap();
			}
		}
	});

	if let Some(user_id) = user_id {
		let connections = user_server_connections(guild_id, user_id)
			.await?;
		if !actions
			.iter()
			.all(|action| action
				.criteria
				.items
				.iter()
				.all(|e| e
					.relevant_connection()
					.map_or(true, |x| connections.iter().any(|e| x == e.kind))
				)
			)
		{
			issues.push(SyncingIssue::MissingConnections);
		}
	}

	Ok(SyncMemberResult {
		initiator,
		issues,
		role_changes,
		member_status,
		profile_changed,
		nickname_change,
		relevant_connections: used_connections,
		user_id: member_id
	})
}

// this needs to move away from recursion
#[async_recursion]
pub async fn member_meets_action_criteria(
	guild_id: Id<GuildMarker>,
	user_id: Option<HakuId<HakuUserMarker>>,
	action: &SyncActionModel,
	all_actions: &Vec<SyncActionModel>,
	connection_metadata: &ConnectionMetadata,
	criteria_cache: &mut HashMap<(HakuId<SyncActionMarker>, usize), bool>,
	used_connections: &mut Vec<HakuId<ConnectionMarker>>
) -> Result<bool> {
	let criteria = &action.criteria;
	let mut total_met = 0;
	let minimum_amount = criteria.quantifier.minimum();
	for (key, item) in criteria.items.iter().enumerate() {
		let cache_key = (action.id, key);
		if criteria_cache.get(&cache_key).is_some_and(|x| *x) || match item {
			CriteriaItem::HakumiUserConnection { connection_kind } => matches!(user_id, Some(user_id) if
				user_server_connections(guild_id, user_id)
					.await?
					.into_iter()
					.any(|x| &x.kind == connection_kind)
			),
			CriteriaItem::PatreonCampaignTierSubscription { tier_id, campaign_id } => if let Some(user_id) = user_id {
				if let Some(pledge) = connection_metadata.patreon_pledges.iter().find(|x| x.user_id == user_id.value && x.campaign_id == *campaign_id && x.tiers.contains(tier_id)) {
					if !used_connections.contains(&pledge.connection_id) {
						used_connections.push(pledge.connection_id);
					}
					true
				} else { false }
			} else { false },
			CriteriaItem::RobloxGroupMembership { .. } |
			CriteriaItem::RobloxGroupMembershipRole { .. } |
			CriteriaItem::RobloxGroupMembershipRoleRankInRange { .. } => if let Some(user_id) = user_id {
				let connections = user_server_connections(guild_id, user_id)
					.await?;
				if let Some(connection) = connections
					.into_iter()
					.find(|x| x.is_roblox())
				{
					if !used_connections.contains(&connection.id) {
						used_connections.push(connection.id);
					}

					let roblox_id = &connection.sub;
					match item {
						CriteriaItem::RobloxGroupMembership { group_id } =>
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && &e.group_id == group_id),
						CriteriaItem::RobloxGroupMembershipRole { role_id, .. } =>
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && &e.role == role_id),
						CriteriaItem::RobloxGroupMembershipRoleRankInRange { group_id, range_lower, range_upper } => {
							connection_metadata.roblox_memberships.iter()
								.any(|e| &e.user_id == roblox_id && &e.group_id == group_id && &e.rank >= range_lower && &e.rank <= range_upper)
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