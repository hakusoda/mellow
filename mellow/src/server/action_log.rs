use serde::{ Serialize, Deserialize };
use serde_json::Value;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	util::unwrap_string_or_array,
	visual_scripting::Document
};

fn v_str(value: &Value) -> String {
	match value {
		Value::String(x) => x.clone(),
		_ => value.to_string()
	}
}

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
	pub author: Option<ActionLogAuthor>,
	pub server_id: Id<GuildMarker>,
	pub data_changes: Vec<DataChange>,
	pub target_action: Option<IdentifiedObject>,
	pub target_command: Option<IdentifiedObject>,
	pub target_webhook: Option<IdentifiedObject>,
	#[serde(skip_serializing)]
	pub target_document: Option<Document>
}

impl ActionLog {
	pub fn action_string(&self, guild_id: &Id<GuildMarker>) -> String {
		match self.kind.as_str() {
			"mellow.server.api_key.created" => "created a new API Key".into(),
			"mellow.server.command.created" => format!("created  {}", self.format_command(guild_id)),
			"mellow.server.command.updated" => format!("updated  {}", self.format_command(guild_id)),
			"mellow.server.command.deleted" => format!("deleted  {}", self.format_command(guild_id)),
			"mellow.server.webhook.created" => format!("created  {}", self.format_webhook(guild_id)),
			"mellow.server.webhook.updated" => format!("updated  {}", self.format_webhook(guild_id)),
			"mellow.server.webhook.deleted" => format!("deleted  {}", self.format_webhook(guild_id)),
			"mellow.server.syncing.action.created" => format!("created  {}", self.format_sync_action(guild_id)),
			"mellow.server.syncing.action.updated" => format!("updated  {}", self.format_sync_action(guild_id)),
			"mellow.server.syncing.action.deleted" => format!("deleted  {}", self.format_sync_action(guild_id)),
			"mellow.server.syncing.settings.updated" => "updated the syncing settings".into(),
			"mellow.server.discord_logging.updated" => "updated the logging settings".into(),
			"mellow.server.ownership.changed" => "transferred ownership to {unimplemented}".into(),
			"mellow.server.automation.event.updated" => format!("updated the {} event", self.data.get("event_name").and_then(|x| x.as_str()).unwrap_or("unknown")),
			"mellow.server.visual_scripting.document.updated" => format!("updated  {}", self.format_document()),
			_ => self.kind.clone()
		}
	}

	pub fn details(&self) -> Vec<String> {
		let mut details: Vec<String> = vec![];

		let is_created = self.kind.ends_with(".created");
		for data_change in self.data_changes.iter() {
			let name = &data_change.name;
			details.push(match &data_change.kind {
				DataChangeKind::Created { value } => if is_created {
					format!("* With {name} **{}**", v_str(value))
				} else { format!("* Set {name} to **{}**", v_str(value)) },
				DataChangeKind::Updated { new_value, old_value } => if name == "name" {
					format!("* Renamed to **{}**, previously **{}**", v_str(new_value), v_str(old_value))
				} else if let Some(value) = new_value.as_bool() {
					format!("* {} {name}", match value {
						true => "Enabled",
						false => "Disabled"
					})
				} else { format!("* Set {name} to **{}**, previously **{}", v_str(new_value), v_str(old_value)) },
				DataChangeKind::Deleted { old_value } => format!("* With {name} of {}", v_str(old_value))
			});
		}

		details
	}

	fn format_command(&self, guild_id: &Id<GuildMarker>) -> String {
		if let Some(command) = &self.target_command {
			format!("<:Command:1226104451065053254> [{}](https://hakumi.cafe/mellow/server/{}/commands/{})", command.name, guild_id, command.id)
		} else {
			format!("<:Command_Deleted:1226110301942972497> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
		}
	}

	fn format_document(&self) -> String {
		if let Some(document) = &self.target_document {
			format!("<:document:1222904218499940395> {}", document.name)
		} else {
			format!("<:document_deleted:1222904235281092638> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
		}
	}

	fn format_sync_action(&self, guild_id: &Id<GuildMarker>) -> String {
		if let Some(action) = &self.target_action {
			format!("<:sync_action:1220987025608413195> [{}](https://hakumi.cafe/mellow/server/{}/syncing/actions/{})", action.name, guild_id, action.id)
		} else {
			format!("<:sync_action_deleted:1220987839328682056> ~~{}~~", self.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
		}
	}
	
	fn format_webhook(&self, guild_id: &Id<GuildMarker>) -> String {
		if let Some(webhook) = &self.target_webhook {
			format!("<:webhook:1220992010975051796> [{}](https://hakumi.cafe/mellow/server/{}/settings/webhooks/{})", webhook.name, guild_id, webhook.id)
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

#[derive(Serialize, Deserialize)]
pub struct DataChange {
	pub name: String,
	#[serde(flatten)]
	pub kind: DataChangeKind
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DataChangeKind {
	Created {
		value: Value
	},
	Updated {
		new_value: Value,
		old_value: Value
	},
	Deleted {
		old_value: Value
	}
}