use std::collections::HashMap;
use serde::Deserialize;
use async_recursion::async_recursion;

use super::{ ServerLog, ProfileSyncKind };
use crate::{
	syncing,
	discord::{ DiscordMember, remove_member },
	database::UserResponse
};

#[derive(Deserialize)]
pub struct Condition {
	pub kind: ConditionKind,
	pub inputs: Vec<EventResponseInput>
}

#[derive(Deserialize)]
pub enum ConditionKind {
	#[serde(rename = "generic.is")]
	GenericIs,
	#[serde(rename = "generic.is_not")]
	GenericIsNot
}

#[derive(Deserialize)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum EventResponseInput {
	Match(serde_json::Value),
	Variable(String)
}

#[derive(Deserialize)]
pub struct ConditionalStatement {
	pub blocks: Vec<ConditionalStatementBlock>
}

#[derive(Deserialize)]
pub struct ConditionalStatementBlock {
	pub items: Vec<EventResponseItem>,
	pub condition: Condition
}

#[derive(Deserialize)]
#[serde(tag = "kind")]
pub enum EventResponseItem {
	#[serde(rename = "action.mellow.sync_profile")]
	SyncMemberProfile,
	#[serde(rename = "action.mellow.member.kick")]
	KickMember,

	#[serde(rename = "statement.if")]
	IfStatement(ConditionalStatement)
}

fn resolve_input(input: &EventResponseInput, variables: &HashMap<String, serde_json::Value>) -> serde_json::Value {
	match input {
		EventResponseInput::Match(value) => value.clone(),
		EventResponseInput::Variable(path) => {
			let mut value: Option<&serde_json::Value> = None;
			for key in path.split("::") {
				if let Some(val) = value {
					value = val.get(key);
				} else {
					value = variables.get(key);
				}
			}

			value.unwrap().clone()
		}
	}
}

pub enum EventResponseResult {
	Complete,
	StopExecution
}

#[async_recursion]
pub async fn start_event_response(items: &Vec<EventResponseItem>, variables: &HashMap<String, serde_json::Value>, server_id: &str, user: Option<&'async_recursion UserResponse>, member: Option<&'async_recursion DiscordMember>) -> EventResponseResult {
	for item in items.iter() {
		match item {
			EventResponseItem::IfStatement(statement) => {
				for block in statement.blocks.iter() {
					let condition = &block.condition;
					if match condition.kind {
						ConditionKind::GenericIs => {
							let input_a = resolve_input(condition.inputs.first().unwrap(), &variables);
							let input_b = resolve_input(condition.inputs.get(1).unwrap(), &variables);
							input_a == input_b
						},
						_ => unimplemented!()
					} {
						match start_event_response(&block.items, &variables, server_id, user, member).await {
							EventResponseResult::StopExecution => return EventResponseResult::StopExecution,
							_ => ()
						}
					}
				}
			},
			EventResponseItem::KickMember => {
				if let Some(member) = member {
					remove_member(server_id, member.id()).await.unwrap();
					
					return EventResponseResult::StopExecution;
				}
			},
			EventResponseItem::SyncMemberProfile => {
				if let Some((user, member)) = user.and_then(|x| member.map(|y| (x, y))) {
					let result = syncing::sync_single_user(&user, &member, server_id, None).await.unwrap();
					if result.profile_changed {
						result.server.send_logs(vec![ServerLog::ServerProfileSync {
							kind: ProfileSyncKind::NewMember,
							member: member.clone(),
							forced_by: None,
							role_changes: result.role_changes.clone(),
							nickname_change: result.nickname_change.clone(),
							relevant_connections: result.relevant_connections.clone()
						}]).await.unwrap();
					}

					return EventResponseResult::StopExecution;
				}
			}
		}
	}

	EventResponseResult::Complete
}