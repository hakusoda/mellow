use std::collections::HashMap;
use uuid::Uuid;
use serde::{ Serialize, Deserialize };
use twilight_model::{
	id::{
		marker::{ RoleMarker, UserMarker, GuildMarker },
		Id
	},
	guild::PartialMember
};
use async_recursion::async_recursion;

use crate::{
	util::WithId,
	model::{
		discord::DISCORD_MODELS,
		hakumi::user::{
			connection::{
				Connection,
				ConnectionKind
			},
			User
		},
		mellow::{
			server::{
				sync_action::{
					SyncAction,
					SyncActionKind,
					RequirementKind,
					RequirementsKind
				},
				Server
			},
			MELLOW_MODELS
		}
	},
	roblox::get_user_group_roles,
	patreon::UserIdentityField,
	discord::CLIENT,
	visual_scripting::{ Variable, DocumentKind },
	Result
};

pub mod sign_ups;

#[derive(Debug, Serialize)]
pub struct SyncMemberResult {
	#[serde(skip)]
	pub server_id: Id<GuildMarker>,
	pub role_changes: Vec<RoleChange>,
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
	pub connection_id: Uuid
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

pub async fn get_connection_metadata(users: &Vec<&User>, server: &Server) -> Result<ConnectionMetadata> {
	let mut patreon_pledges: Vec<PatreonPledge> = vec![];
	let mut roblox_memberships: Vec<RobloxMembership> = vec![];
	let mut group_ids: Vec<String> = vec![];
	for action in &server.actions {
		for requirement in &action.requirements {
			match requirement.kind {
				RequirementKind::RobloxHaveGroupRole |
				RequirementKind::RobloxHaveGroupRankInRange |
				RequirementKind::RobloxInGroup => {
					let id = requirement.data.first().unwrap();
					if !group_ids.contains(&id) {
						group_ids.push(id.clone());
					}
				},
				RequirementKind::PatreonHaveCampaignTier => {
					for user in users {
						if let Some(connection) = user.server_connections().into_iter().find(|x| matches!(x.kind, ConnectionKind::Patreon)) {
							let data = crate::patreon::get_user_memberships(&connection.oauth_authorisations.as_ref().unwrap()[0]).await?;
							if let Some(included) = data.included {
								for membership in included {
									match membership {
										UserIdentityField::Member(member) => patreon_pledges.push(PatreonPledge {
											tiers: member.relationships.currently_entitled_tiers.data.0.iter().map(|x| x.id.clone()).collect(),
											active: member.attributes.patron_status.is_some_and(|x| x == "active_patron"),
											user_id: user.id.clone(),
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
		//let roblox_ids: Vec<String> = users.iter().flat_map(|x| x.user.connections.iter().filter(|x| matches!(x.connection.kind, ConnectionKind::Roblox)).map(|x| format!("users/{}", x.connection.sub)).collect::<Vec<String>>()).collect();
		//let items = get_group_memberships("-", Some(format!("user in ['{}']", roblox_ids.join("','")))).await;
		for id in users.iter().flat_map(|x| x.server_connections().into_iter().filter(|x| matches!(x.kind, ConnectionKind::Roblox)).map(|x| x.sub.clone()).collect::<Vec<String>>()) {
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

#[tracing::instrument(level = "trace")]
async fn get_role_name(guild_id: Id<GuildMarker>, role_id: Id<RoleMarker>) -> Result<String> {
	DISCORD_MODELS.role(guild_id, role_id).await.map(|x| x.name.clone())
	//return Ok(items.iter().find(|x| &x.id == id).map_or("unknown role".into(), |x| x.name.clone()));
}

// async_recursion required due to a cycle error caused by visual scripting
#[async_recursion]
#[tracing::instrument(level = "trace")]
pub async fn sync_single_user(server: &Server, user: &User, member: &WithId<Id<UserMarker>, PartialMember>, connection_metadata: Option<ConnectionMetadata>) -> Result<SyncMemberResult> {
	let metadata = match connection_metadata {
		Some(x) => x,
		None => get_connection_metadata(&vec![user], &server).await?
	};
	sync_member(Some(user), &member, &server, &metadata).await
}

#[tracing::instrument(level = "trace")]
pub async fn sync_member(user: Option<&User>, member: &WithId<Id<UserMarker>, PartialMember>, server: &Server, connection_metadata: &ConnectionMetadata) -> Result<SyncMemberResult> {
	let mut roles = member.roles.clone();
	let mut role_changes: Vec<RoleChange> = vec![];
	let mut requirement_cache: HashMap<String, bool> = HashMap::new();
	let mut used_connections: Vec<Connection> = vec![];
	for action in server.actions.iter() {
		let met = member_meets_action_requirements(user, action, &server.actions, &connection_metadata, &mut requirement_cache, &mut used_connections).await;
		match action.kind {
			SyncActionKind::GiveRoles => {
				let items: Vec<Id<RoleMarker>> = action.metadata["items"].as_array().unwrap().iter().map(|x| Id::new(x.as_str().unwrap().parse().unwrap())).collect();
				if met {
					if !items.iter().all(|x| member.roles.iter().any(|e| e == x)) {
						let filtered: Vec<Id<RoleMarker>> = items.into_iter().filter(|x| !member.roles.iter().any(|e| e == x)).collect();
						for role_id in filtered {
							roles.push(role_id);
							role_changes.push(RoleChange {
								kind: RoleChangeKind::Added,
								target_id: role_id,
								display_name: get_role_name(server.id, role_id).await?
							});
						}
					}
				} else if action.metadata["can_remove"].as_bool().unwrap() {
					let filtered: Vec<Id<RoleMarker>> = roles.clone().into_iter().filter(|x| !items.contains(x)).collect();
					if !roles.iter().all(|x| filtered.contains(x)) {
						for role_id in items {
							if roles.contains(&role_id) {
								role_changes.push(RoleChange {
									kind: RoleChangeKind::Removed,
									target_id: role_id,
									display_name: get_role_name(server.id, role_id).await?
								});
							}
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
				user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, ConnectionKind::Roblox)).and_then(|x| x.username.as_ref())),
			"{roblox_display_name}" =>
				user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, ConnectionKind::Roblox)).and_then(|x| x.display_name.as_ref())),
			_ => None
		}.map(|x| x.as_str()),
		None => None
	};
	let nickname_change = if let Some(target) = &target_nickname {
		if member.nick.is_none() || member.nick.clone().is_some_and(|x| x != *target) {
			Some(NicknameChange(member.nick.clone(), Some(target.to_string())))
		} else { None }
	} else { None };
	
	let profile_changed = !role_changes.is_empty() || nickname_change.is_some();
	if profile_changed {
		let mut request = CLIENT.update_guild_member(server.id, member.id);
		if !role_changes.is_empty() {
			request = request.roles(&roles);
		}
		if nickname_change.is_some() {
			request = request.nick(target_nickname)?;
		}
		request.await?;
	}

	/*if !used_connections.is_empty() {
		let connection_ids: Vec<String> = used_connections.iter().map(|x| x.id.to_string()).collect();
		todo!();
	}*/

	// TODO: better.
	let member = member.inner.clone();
	let guild_id = server.id.clone();
	let role_changes2 = role_changes.clone();
	tokio::spawn(async move {
		let document = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberSynced).await.unwrap();
		if document.is_ready_for_stream() {
			let variables = Variable::create_map([
				("member", Variable::from_partial_member(None, &member, &guild_id)),
				("guild_id", guild_id.to_string().into()),
				("profile_changes", Variable::create_map([
					("roles", Variable::create_map([
						("added", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Added) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into()),
						("removed", role_changes2.iter().filter_map(|x| if matches!(x.kind, RoleChangeKind::Removed) { Some(x.target_id) } else { None }).collect::<Vec<Id<RoleMarker>>>().into())
					], None))
				], None))
			], None);
			document
				.process(variables)
				.await.unwrap()
				.send_logs(guild_id)
				.await.unwrap();
		}
	});

	let is_missing_connections = user.is_some_and(|user| !server.actions.iter().all(|x| x.requirements.iter().all(|e| e.relevant_connection().map_or(true, |x| user.server_connections().into_iter().any(|e| x == e.kind)))));
	Ok(SyncMemberResult {
		server_id: server.id,
		role_changes,
		profile_changed,
		nickname_change,
		relevant_connections: used_connections,
		is_missing_connections
	})
}

#[async_recursion]
pub async fn member_meets_action_requirements(
	user: Option<&'async_recursion User>,
	action: &SyncAction,
	all_actions: &Vec<SyncAction>,
	connection_metadata: &ConnectionMetadata,
	cache: &mut HashMap<String, bool>,
	used_connections: &mut Vec<Connection>
) -> bool {
	let mut total_met = 0;
	let requires_one = matches!(action.requirements_type, RequirementsKind::MeetOne);
	for item in action.requirements.iter() {
		if cache.get(&item.id).is_some_and(|x| *x) || match item.kind {
			RequirementKind::RobloxHaveConnection |
			RequirementKind::RobloxInGroup |
			RequirementKind::RobloxHaveGroupRole |
			RequirementKind::RobloxHaveGroupRankInRange => {
				let connection = user.and_then(|x| x.server_connections().into_iter().find(|x| matches!(x.kind, ConnectionKind::Roblox)));
				if let Some(connection) = connection{
					if !used_connections.contains(connection) {
						used_connections.push(connection.clone());
					}
				}

				match item.kind {
					RequirementKind::RobloxHaveConnection =>
						connection.is_some(),
					RequirementKind::RobloxInGroup =>
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.group_id.to_string() == item.data[0])),
					RequirementKind::RobloxHaveGroupRole =>
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.role.to_string() == item.data[1])),
					RequirementKind::RobloxHaveGroupRankInRange => {
						let id = &item.data[0];
						let min: u8 = item.data[1].parse().unwrap();
						let max: u8 = item.data[2].parse().unwrap();
						connection.map_or(false, |x| connection_metadata.roblox_memberships.iter().any(|e| e.user_id == x.sub && e.group_id.to_string() == *id && e.rank >= min && e.rank <= max))
					},
					_ => false
				}
			},
			RequirementKind::MeetOtherAction => {
				let target_id = &item.data[0];
				if let Some(action2) = all_actions.iter().find(|x| x.id.to_string() == *target_id) {
					member_meets_action_requirements(user, action2, &all_actions, &connection_metadata, cache, used_connections).await
				} else { false }
			},
			RequirementKind::PatreonHaveCampaignTier => {
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