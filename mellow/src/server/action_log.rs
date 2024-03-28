use serde::{ Serialize, Deserialize };

use crate::{
	util::unwrap_string_or_array,
	visual_scripting::Document
};

#[derive(Serialize, Deserialize)]
pub struct IdentifiedObject {
	pub id: String,
	pub name: String
}

#[derive(Serialize, Deserialize)]
pub struct ActionLog {
	#[serde(rename = "type")]
	pub kind: String,
	pub data: serde_json::Value,
	pub author: ActionLogAuthor,
	pub server_id: String,
	pub target_action: Option<IdentifiedObject>,
	pub target_webhook: Option<IdentifiedObject>,
	#[serde(skip_serializing)]
	pub target_document: Option<Document>
}

impl ActionLog {
	pub fn action_string(&self, server_id: impl Into<String>) -> String {
		let server_id: String = server_id.into();
		match self.kind.as_str() {
			"mellow.server.api_key.created" => "created a new API Key".into(),
			"mellow.server.webhook.created" => format!("created  {}", self.format_webhook(server_id)),
			"mellow.server.webhook.updated" => format!("updated  {}", self.format_webhook(server_id)),
			"mellow.server.webhook.deleted" => format!("deleted  {}", self.format_webhook(server_id)),
			"mellow.server.syncing.action.created" => format!("created  {}", self.format_sync_action(server_id)),
			"mellow.server.syncing.action.updated" => format!("updated  {}", self.format_sync_action(server_id)),
			"mellow.server.syncing.action.deleted" => format!("deleted  {}", self.format_sync_action(server_id)),
			"mellow.server.syncing.settings.updated" => "updated the syncing settings".into(),
			"mellow.server.discord_logging.updated" => "updated the logging settings".into(),
			"mellow.server.ownership.changed" => "transferred ownership to {unimplemented}".into(),
			"mellow.server.automation.event.updated" => format!("updated the {} event", self.data.get("event_name").and_then(|x| x.as_str()).unwrap_or("unknown")),
			"mellow.server.visual_scripting.document.updated" => format!("updated the {} visual scripting document", self.format_document()),
			_ => self.kind.clone()
		}
	}

	fn format_document(&self) -> String {
		if let Some(document) = &self.target_document {
			format!("<:document:1222904218499940395> {}]", document.name)
		} else {
			format!("<:document_deleted:1222904235281092638> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
		}
	}

	fn format_sync_action(&self, server_id: impl Into<String>) -> String {
		if let Some(action) = &self.target_action {
			format!("<:sync_action:1220987025608413195> [{}](https://hakumi.cafe/mellow/server/{}/syncing/actions/{})", action.name, server_id.into(), action.id)
		} else {
			format!("<:sync_action_deleted:1220987839328682056> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
		}
	}
	
	fn format_webhook(&self, server_id: impl Into<String>) -> String {
		if let Some(webhook) = &self.target_webhook {
			format!("<:webhook:1220992010975051796> [{}](https://hakumi.cafe/mellow/server/{}/settings/webhooks/{})", webhook.name, server_id.into(), webhook.id)
		} else {
			format!("<:webhook_deleted:1220992273525772309> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Webhook"))
		}
	}
}

#[derive(Serialize, Deserialize)]
pub struct ActionLogAuthor {
	pub id: String,
	pub name: Option<String>,
	pub username: String,
	pub avatar_url: Option<String>
}

impl ActionLogAuthor {
	pub fn display_name(&self) -> String {
		self.name.as_ref().map_or_else(|| self.username.clone(), |x| x.clone())
	}
}