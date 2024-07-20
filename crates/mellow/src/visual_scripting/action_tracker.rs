use mellow_models::hakumi::visual_scripting::ElementKind;
use twilight_model::id::{
	marker::{ GuildMarker, ChannelMarker, MessageMarker },
	Id
};

use crate::{
	error::Error,
	server::logging::{ ServerLog, send_logs },
	Result
};

pub struct ActionTracker {
	document_name: String,
	items: Vec<ActionTrackerItem>,
	pub replied: bool
}

impl ActionTracker {
	pub fn new(document_name: String) -> Self {
		Self {
			document_name,
			items: Vec::new(),
			replied: false
		}
	}

	pub async fn send_logs(self, guild_id: Id<GuildMarker>) -> Result<()> {
		if !self.items.is_empty() {
			send_logs(guild_id, vec![ServerLog::VisualScriptingDocumentResult {
				items: self.items,
				document_name: self.document_name
			}])
				.await?;
		}
		Ok(())
	}

	pub fn error(&mut self, element_kind: ElementKind, source: Error) {
		self.items.push(ActionTrackerItem::Error(element_kind, source));
	}

	pub fn created_thread(&mut self, channel_id: Id<ChannelMarker>, thread_id: Id<ChannelMarker>) {
		self.items.push(ActionTrackerItem::CreatedThread(channel_id, thread_id));
	}

	pub fn assigned_member_role(&mut self, user_id: impl ToString, role_id: impl ToString) {
		self.items.push(ActionTrackerItem::AssignedMemberRole(user_id.to_string(), role_id.to_string()));
	}

	pub fn removed_member_role(&mut self, user_id: impl ToString, role_id: impl ToString) {
		self.items.push(ActionTrackerItem::RemovedMemberRole(user_id.to_string(), role_id.to_string()));
	}

	pub fn banned_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::BannedMember(user_id.to_string()));
	}

	pub fn kicked_member(&mut self, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::KickedMember(user_id.to_string()));
	}

	pub fn created_message(&mut self, channel_id: Id<ChannelMarker>, message_id: Id<MessageMarker>) {
		self.items.push(ActionTrackerItem::CreatedMessage(channel_id, message_id));
	}

	pub fn deleted_message(&mut self, channel_id: impl ToString, user_id: impl ToString) {
		self.items.push(ActionTrackerItem::DeletedMessage(channel_id.to_string(), user_id.to_string()));
	}
}

pub enum ActionTrackerItem {
	Error(ElementKind, Error),
	AssignedMemberRole(String, String),
	RemovedMemberRole(String, String),
	BannedMember(String),
	KickedMember(String),
	CreatedMessage(Id<ChannelMarker>, Id<MessageMarker>),
	DeletedMessage(String, String),
	CreatedThread(Id<ChannelMarker>, Id<ChannelMarker>)
}