use serde::{ Serialize, Deserialize };
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	server::{ logging::ServerLog, Server },
	Result
};

pub struct ActionTracker {
	items: Vec<ActionTrackerItem>,
	document_name: String
}

impl ActionTracker {
	pub fn new(document_name: String) -> Self {
		Self {
			items: vec![],
			document_name
		}
	}

	pub async fn send_logs(self, guild_id: &Id<GuildMarker>) -> Result<()> {
		if !self.items.is_empty() {
			let server = Server::fetch(guild_id).await?;
			server.send_logs(vec![ServerLog::VisualScriptingDocumentResult {
				items: self.items,
				document_name: self.document_name
			}]).await?;
		}
		Ok(())
	}

	pub fn assigned_member_role(&mut self, user_id: impl ToString, role_id: impl ToString) {
		self.items.push(ActionTrackerItem::AssignedMemberRole(user_id.to_string(), role_id.to_string()));
	}

	pub fn banned_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::BannedMember(user_id.to_string()));
	}

	pub fn kicked_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::KickedMember(user_id.to_string()));
	}

	pub fn deleted_message(&mut self, channel_id: impl ToString, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::DeletedMessage(channel_id.to_string(), user_id.to_string()));
	}
}

#[derive(Serialize, Deserialize)]
pub enum ActionTrackerItem {
	AssignedMemberRole(String, String),
	BannedMember(String),
	KickedMember(String),
	DeletedMessage(String, String)
}