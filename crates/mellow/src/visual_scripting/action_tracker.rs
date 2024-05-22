use twilight_model::id::{
	marker::{ GuildMarker, ChannelMarker, MessageMarker },
	Id
};

use crate::{
	error::Error,
	model::mellow::MELLOW_MODELS,
	server::logging::ServerLog,
	visual_scripting::ElementKind,
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

	pub async fn send_logs(self, guild_id: Id<GuildMarker>) -> Result<()> {
		if !self.items.is_empty() {
			let server = MELLOW_MODELS.server(guild_id).await?;
			server.send_logs(vec![ServerLog::VisualScriptingDocumentResult {
				items: self.items,
				document_name: self.document_name
			}]).await?;
		}
		Ok(())
	}

	pub fn error(&mut self, element_kind: ElementKind, source: Error) {
		self.items.push(ActionTrackerItem::Error(element_kind, source));
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
	DeletedMessage(String, String)
}