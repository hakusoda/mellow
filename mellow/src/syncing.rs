use std::time::SystemTime;
use std::collections::HashMap;
use serde::{ Serialize, Deserialize };
use tokio::sync::RwLock;
use async_recursion::async_recursion;

use crate::{
	roblox::get_user_group_roles,
	discord::{ DiscordMember, DiscordModifyMemberPayload, get_member, modify_member },
	commands,
	database::{ User, Server, UserResponse, UserConnection, ProfileSyncAction, UserConnectionKind, ProfileSyncActionKind, ProfileSyncActionRequirement, ProfileSyncActionRequirementKind, ProfileSyncActionRequirementsKind, get_users_by_discord }
};

#[derive(Debug)]
pub struct SyncMemberResult {
	pub role_changes: Vec<RoleChange>,
	pub profile_changed: bool,
	pub relevant_connections: Vec<UserConnection>
}

#[derive(Debug, Deserialize, Serialize)]
pub struct RoleChange {
	pub kind: RoleChangeKind,
	pub target_id: String,
	pub display_name: String
}

#[derive(Debug, Deserialize, Serialize)]
pub enum RoleChangeKind {
	Added,
	Removed
}

#[derive(Debug)]
pub struct RobloxMembership {
	pub rank: u8,
	pub role: u128,
	pub user_id: String,
	pub group_id: u128
}

#[derive(Debug)]
pub struct ConnectionMetadata {
	pub roblox_memberships: Vec<RobloxMembership>
}

pub async fn get_connection_metadata(users: &[UserResponse], server: &Server) -> ConnectionMetadata {
	let mut roblox_memberships: Vec<RobloxMembership> = vec![];
	let mut group_ids: Vec<String> = vec![];
	for action in server.actions.iter() {
		for requirement in action.requirements.iter() {
			match requirement.kind {
				ProfileSyncActionRequirementKind::RobloxHaveGroupRole |
				ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange |
				ProfileSyncActionRequirementKind::RobloxInGroup => {
					let id = requirement.data.first().unwrap();
					if !group_ids.contains(&id) {
						group_ids.push(id.clone());
					}
				},
				_ => {}
			}
		}
	}

	if !group_ids.is_empty() {
		//let roblox_ids: Vec<String> = users.iter().flat_map(|x| x.user.connections.iter().filter(|x| matches!(x.connection.kind, UserConnectionKind::Roblox)).map(|x| format!("users/{}", x.connection.sub)).collect::<Vec<String>>()).collect();
		//let items = get_group_memberships("-", Some(format!("user in ['{}']", roblox_ids.join("','")))).await;
		for id in users.iter().flat_map(|x| x.user.connections.iter().filter(|x| matches!(x.connection.kind, UserConnectionKind::Roblox)).map(|x| x.connection.sub.clone()).collect::<Vec<String>>()) {
			let roles = get_user_group_roles(&id).await;
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

	ConnectionMetadata {
		roblox_memberships
	}
}

pub async fn sync_member(user: Option<&User>, member: &DiscordMember, server: &Server, connection_metadata: &ConnectionMetadata) -> SyncMemberResult {
	let mut roles = member.roles.clone();
	let mut role_changes: Vec<RoleChange> = vec![];
	let mut requirement_cache: HashMap<ProfileSyncActionRequirement, bool> = HashMap::new();
	let mut used_connections: Vec<UserConnection> = vec![];
	
	let actions2 = server.actions.clone();
	for action in server.actions.iter() {
		let met = member_meets_action_requirements(user, action, &actions2, &connection_metadata, &mut requirement_cache, &mut used_connections).await;
		match action.kind {
			ProfileSyncActionKind::GiveRoles => {
				let items: Vec<String> = action.metadata["items"].as_array().unwrap().iter().map(|x| x.as_str().unwrap().to_string()).collect();
				if met {
					if !items.iter().all(|x| member.roles.iter().any(|e| e == x)) {
						let filtered: Vec<String> = items.into_iter().filter(|x| !member.roles.iter().any(|e| e == x)).collect();
						for item in filtered {
							roles.push(item.clone());
							role_changes.push(RoleChange {
								kind: RoleChangeKind::Added,
								target_id: item,
								display_name: "placeholder name".into()
							});
						}
					}
				} else if action.metadata["can_remove"].as_bool().unwrap() {
					let filtered: Vec<String> = roles.clone().into_iter().filter(|x| !items.contains(x)).collect();
					if !roles.iter().all(|x| filtered.contains(x)) {
						let filtered2 = items.iter().filter(|x| roles.contains(x));
						for item in filtered2 {
							role_changes.push(RoleChange {
								kind: RoleChangeKind::Removed,
								target_id: item.clone(),
								display_name: "placeholder name".into()
							});
						}
						roles = filtered;
					}
				}
			},
			_ => {}
		};
	}
	let profile_changed = !role_changes.is_empty();
	if profile_changed {
		modify_member(server.id.clone(), member.user.id.clone(), DiscordModifyMemberPayload {
			roles: Some(roles),
			..Default::default()
		}).await;
	}

	SyncMemberResult {
		role_changes,
		profile_changed,
		relevant_connections: used_connections
	}
}

#[async_recursion]
pub async fn member_meets_action_requirements(
	user: Option<&'async_recursion User>,
	action: &ProfileSyncAction,
	all_actions: &Vec<ProfileSyncAction>,
	connection_metadata: &ConnectionMetadata,
	cache: &mut HashMap<ProfileSyncActionRequirement, bool>,
	used_connections: &mut Vec<UserConnection>
) -> bool {
	let mut total_met = 0;
	let requires_one = matches!(action.requirements_type, ProfileSyncActionRequirementsKind::MeetOne);
	for item in action.requirements.iter() {
		if if let Some(cached) = cache.get(item) {
			*cached
		} else {
			match item.kind {
				ProfileSyncActionRequirementKind::RobloxHaveConnection |
				ProfileSyncActionRequirementKind::RobloxInGroup |
				ProfileSyncActionRequirementKind::RobloxHaveGroupRole |
				ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange => {
					let connection = user.and_then(|x| x.connections.iter().find(|x| matches!(x.connection.kind, UserConnectionKind::Roblox)));
					if let Some(connection) = connection.cloned() {
						if !used_connections.contains(&connection.connection) {
							used_connections.push(connection.connection);
						}
					}

					return match item.kind {
						ProfileSyncActionRequirementKind::RobloxHaveConnection =>
							connection.is_some(),
						ProfileSyncActionRequirementKind::RobloxInGroup =>
							connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.connection.sub && e.group_id.to_string() == item.data[0])),
						ProfileSyncActionRequirementKind::RobloxHaveGroupRole =>
							connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.connection.sub && e.role.to_string() == item.data[1])),
						ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange => {
							let id = &item.data[0];
							let min: u8 = item.data[1].parse().unwrap();
							let max: u8 = item.data[2].parse().unwrap();
							connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.connection.sub && e.group_id.to_string() == *id && e.rank >= min && e.rank <= max))
						},
						_ => false
					};
				},
				ProfileSyncActionRequirementKind::MeetOtherAction => {
					for action2 in all_actions.iter() {
						if action2.id == item.data[0] {
							return member_meets_action_requirements(user, action2, &all_actions, &connection_metadata, cache, used_connections).await;
						}
					}
					false
				},
				_ => false
			}
		} {
			cache.insert(item.clone(), true);
			total_met += 1;
			if requires_one {
				return true;
			}
		} else {
			cache.insert(item.clone(), false);
		}
	}
	total_met == action.requirements.len()
}

struct SignUp {
	pub user_id: String,
	pub guild_id: String,
	pub created_at: SystemTime,
	pub interaction_token: String
}

static SIGN_UPS: RwLock<Vec<SignUp>> = RwLock::const_new(vec![]);

pub async fn create_sign_up(user_id: String, guild_id: String, interaction_token: String) {
	let mut items = SIGN_UPS.write().await;
	if let Some(existing) = items.iter_mut().find(|x| x.user_id == user_id) {
		existing.guild_id = guild_id;
		existing.created_at = SystemTime::now();
		existing.interaction_token = interaction_token;
	} else {
		items.push(SignUp {
			user_id,
			guild_id,
			created_at: SystemTime::now(),
			interaction_token
		});
	}
}

pub async fn finish_sign_up(discord_id: String) {
	if let Some(item) = SIGN_UPS.read().await.iter().find(|x| x.user_id == discord_id) {
		if SystemTime::now().duration_since(item.created_at).unwrap().as_secs() < 891 {
			if let Some(user) = get_users_by_discord(vec![discord_id.clone()], item.guild_id.clone()).await.into_iter().next() {
				let member = get_member(&item.guild_id, &discord_id).await;
				commands::syncing::sync_with_token(user, member, &item.guild_id, &item.interaction_token).await;
			}
		}
	}
	SIGN_UPS.write().await.retain(|x| x.user_id != discord_id);
}