use std::collections::HashMap;
use serde::{ Serialize, Deserialize };
use async_recursion::async_recursion;

use crate::{
	server::Server,
	patreon::UserIdentityField,
	database::{ ProfileSyncAction, ProfileSyncActionKind, ProfileSyncActionRequirementKind, ProfileSyncActionRequirementsKind, User, UserConnection, UserConnectionKind, UserResponse, DATABASE }, discord::{ get_guild_roles, modify_member, DiscordMember, DiscordModifyMemberPayload, DiscordRole }, roblox::get_user_group_roles, Result
};

pub mod sign_ups;

#[derive(Debug, Serialize)]
pub struct SyncMemberResult {
	#[serde(skip)]
	pub server: Server,
	pub role_changes: Vec<RoleChange>,
	pub profile_changed: bool,
	pub nickname_change: Option<NicknameChange>,
	pub relevant_connections: Vec<UserConnection>
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RoleChange {
	pub kind: RoleChangeKind,
	pub target_id: String,
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
	pub user_id: String,
	pub campaign_id: String,
	pub connection_id: String
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
	pub patreon_pledges: Vec<PatreonPledge>,
	pub roblox_memberships: Vec<RobloxMembership>
}

pub async fn get_connection_metadata(users: &[UserResponse], server: &Server) -> Result<ConnectionMetadata> {
	let mut patreon_pledges: Vec<PatreonPledge> = vec![];
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
				ProfileSyncActionRequirementKind::PatreonHaveCampaignTier => {
					for user in users.iter() {
						if let Some(connection) = user.user.server_connections().into_iter().find(|x| matches!(x.kind, UserConnectionKind::Patreon)) {
							let data = crate::patreon::get_user_memberships(&connection.oauth_authorisations.as_ref().unwrap()[0]).await?;
							if let Some(included) = data.included {
								for membership in included {
									match membership {
										UserIdentityField::Member(member) => patreon_pledges.push(PatreonPledge {
											tiers: member.relationships.currently_entitled_tiers.data.0.iter().map(|x| x.id.clone()).collect(),
											active: member.attributes.patron_status.is_some_and(|x| x == "active_patron"),
											user_id: user.user.id.clone(),
											campaign_id: member.relationships.campaign.data.id,
											connection_id: connection.id.clone()
										}),
										_ => ()
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
		//let roblox_ids: Vec<String> = users.iter().flat_map(|x| x.user.connections.iter().filter(|x| matches!(x.connection.kind, UserConnectionKind::Roblox)).map(|x| format!("users/{}", x.connection.sub)).collect::<Vec<String>>()).collect();
		//let items = get_group_memberships("-", Some(format!("user in ['{}']", roblox_ids.join("','")))).await;
		for id in users.iter().flat_map(|x| x.user.server_connections().into_iter().filter(|x| matches!(x.kind, UserConnectionKind::Roblox)).map(|x| x.sub.clone()).collect::<Vec<String>>()) {
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

async fn get_role_name(id: String, guild_id: impl Into<String>, roles: &mut Option<Vec<DiscordRole>>) -> Result<String> {
	let items = match roles {
		Some(x) => x,
		None => {
			*roles = Some(get_guild_roles(guild_id).await?);
			roles.as_ref().unwrap()
		}
	};
	return Ok(items.iter().find(|x| x.id == id).map_or("unknown role".into(), |x| x.name.clone()));
}

pub async fn sync_single_user(user: &UserResponse, member: &DiscordMember, guild_id: impl Into<String>, connection_metadata: Option<ConnectionMetadata>) -> Result<SyncMemberResult> {
	let server = Server::fetch(guild_id).await?;
	let metadata = match connection_metadata {
		Some(x) => x,
		None => get_connection_metadata(&vec![user.clone()], &server).await?
	};
	sync_member(Some(&user.user), &member, &server, &metadata, &mut None).await
}

pub async fn sync_member(user: Option<&User>, member: &DiscordMember, server: &Server, connection_metadata: &ConnectionMetadata, guild_roles: &mut Option<Vec<DiscordRole>>) -> Result<SyncMemberResult> {
	let mut roles = member.roles.clone();
	let mut role_changes: Vec<RoleChange> = vec![];
	let mut requirement_cache: HashMap<String, bool> = HashMap::new();
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
								target_id: item.clone(),
								display_name: get_role_name(item, &server.id, guild_roles).await?
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
								display_name: get_role_name(item.clone(), &server.id, guild_roles).await?
							});
						}
						roles = filtered;
					}
				}
			},
			_ => {}
		};
	}

	let target_nickname = match &server.default_nickname {
		Some(t) => match t.as_str() {
			"{roblox_username}" =>
				user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, UserConnectionKind::Roblox)).and_then(|x| x.username.clone())),
			"{roblox_display_name}" =>
				user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, UserConnectionKind::Roblox)).and_then(|x| x.display_name.clone())),
			_ => None
		},
		None => None
	};
	let nickname_change = if let Some(target) = &target_nickname {
		if member.nick.is_none() || member.nick.clone().is_some_and(|x| x != *target) {
			Some(NicknameChange(member.nick.clone(), Some(target.clone())))
		} else { None }
	} else { None };
	
	let profile_changed = !role_changes.is_empty() || nickname_change.is_some();
	if profile_changed {
		modify_member(server.id.clone(), member.id(), DiscordModifyMemberPayload {
			nick: target_nickname,
			roles: Some(roles),
			..Default::default()
		}).await?;
	}

	if !used_connections.is_empty() {
		let connection_ids: Vec<String> = used_connections.iter().map(|x| x.id.clone()).collect();
		tokio::spawn(async move {
			DATABASE
				.from("mellow_user_server_connections")
				.update(format!(r#"{{ "last_used_at": "{}" }}"#, chrono::Local::now()))
				.in_("id", connection_ids)
				.execute()
				.await
				.unwrap();
		});
	}

	Ok(SyncMemberResult {
		server: server.clone(),
		role_changes,
		profile_changed,
		nickname_change,
		relevant_connections: used_connections
	})
}

#[async_recursion]
pub async fn member_meets_action_requirements(
	user: Option<&'async_recursion User>,
	action: &ProfileSyncAction,
	all_actions: &Vec<ProfileSyncAction>,
	connection_metadata: &ConnectionMetadata,
	cache: &mut HashMap<String, bool>,
	used_connections: &mut Vec<UserConnection>
) -> bool {
	let mut total_met = 0;
	let requires_one = matches!(action.requirements_type, ProfileSyncActionRequirementsKind::MeetOne);
	for item in action.requirements.iter() {
		if cache.get(&item.id).is_some_and(|x| *x) || match item.kind {
			ProfileSyncActionRequirementKind::RobloxHaveConnection |
			ProfileSyncActionRequirementKind::RobloxInGroup |
			ProfileSyncActionRequirementKind::RobloxHaveGroupRole |
			ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange => {
				let connection = user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, UserConnectionKind::Roblox)));
				if let Some(connection) = connection{
					if !used_connections.contains(connection) {
						used_connections.push(connection.clone());
					}
				}

				match item.kind {
					ProfileSyncActionRequirementKind::RobloxHaveConnection =>
						connection.is_some(),
					ProfileSyncActionRequirementKind::RobloxInGroup =>
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.group_id.to_string() == item.data[0])),
					ProfileSyncActionRequirementKind::RobloxHaveGroupRole =>
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.role.to_string() == item.data[1])),
					ProfileSyncActionRequirementKind::RobloxHaveGroupRankInRange => {
						let id = &item.data[0];
						let min: u8 = item.data[1].parse().unwrap();
						let max: u8 = item.data[2].parse().unwrap();
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.group_id.to_string() == *id && e.rank >= min && e.rank <= max))
					},
					_ => false
				}
			},
			ProfileSyncActionRequirementKind::MeetOtherAction => {
				let target_id = &item.data[0];
				if let Some(action2) = all_actions.iter().find(|x| x.id == *target_id) {
					member_meets_action_requirements(user, action2, &all_actions, &connection_metadata, cache, used_connections).await
				} else { false }
			},
			ProfileSyncActionRequirementKind::PatreonHaveCampaignTier => {
				let campaign_id = &item.data[0];
				let tier_id = &item.data[1];
				if let Some(user) = user {
					if let Some(pledge) = connection_metadata.patreon_pledges.iter().find(|x| x.active && x.user_id == user.id && x.campaign_id == *campaign_id && x.tiers.contains(tier_id)) {
						if let Some(connection) = user.server_connections().into_iter().find(|x| x.id == pledge.connection_id) {
							if !used_connections.contains(connection) {
								used_connections.push(connection.clone());
							}
						}
						true
					} else { false }
				} else { false }
			},
			_ => false
		} {
			cache.insert(item.id.clone(), true);
			if requires_one {
				return true;
			}
			total_met += 1;
		} else {
			cache.insert(item.id.clone(), false);
		}
	}
	total_met == action.requirements.len()
}