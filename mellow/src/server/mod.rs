use serde::{ Serialize, Deserialize };

use crate::{
	discord::{ DiscordMember, ChannelMessage, create_channel_message },
	syncing::{ RoleChange, NicknameChange, RoleChangeKind },
	database::{ Server, UserConnection },
	interaction::{ Embed, EmbedField, EmbedAuthor },
	Result
};

pub enum ProfileSyncKind {
	Default,
	NewMember
}

impl Default for ProfileSyncKind {
	fn default() -> Self {
		Self::Default
	}
}

#[derive(Deserialize, Serialize)]
pub struct ActionLog {
	#[serde(rename = "type")]
	pub kind: String,
	pub data: serde_json::Value,
	pub author: ActionLogAuthor,
	pub server_id: String,
	pub target_action: Option<IdentifiedObject>,
	pub target_webhook: Option<IdentifiedObject>
}

#[derive(Deserialize, Serialize)]
pub struct IdentifiedObject {
	pub id: String,
	pub name: String
}

fn unwrap_string_or_array(value: &serde_json::Value) -> Option<&str> {
	value.as_array().map_or_else(|| value.as_str(), |x| x.get(0).and_then(|x| x.as_str()))
}

fn format_sync_action(action_log: &ActionLog, server_id: impl Into<String>) -> String {
	if let Some(action) = &action_log.target_action {
		format!("<:sync_action:1220987025608413195> [{}](https://hakumi.cafe/mellow/server/{}/syncing/actions/{})", action.name, server_id.into(), action.id)
	} else {
		format!("<:sync_action_deleted:1220987839328682056> ~~{}~~", action_log.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Action"))
	}
}

fn format_webhook(action_log: &ActionLog, server_id: impl Into<String>) -> String {
	if let Some(webhook) = &action_log.target_webhook {
		format!("<:webhook:1220992010975051796> [{}](https://hakumi.cafe/mellow/server/{}/settings/webhooks/{})", webhook.name, server_id.into(), webhook.id)
	} else {
		format!("<:webhook_deleted:1220992273525772309> ~~{}~~", action_log.data.get("name").and_then(unwrap_string_or_array).unwrap_or("Unknown Webhook"))
	}
}

impl ActionLog {
	pub fn action_string(&self, server_id: impl Into<String>) -> String {
		let server_id: String = server_id.into();
		match self.kind.as_str() {
			"mellow.server.api_key.created" => "created a new API Key".into(),
			"mellow.server.webhook.created" => format!("created  {}", format_webhook(&self, server_id)),
			"mellow.server.webhook.updated" => format!("updated  {}", format_webhook(&self, server_id)),
			"mellow.server.webhook.deleted" => format!("deleted  {}", format_webhook(&self, server_id)),
			"mellow.server.syncing.action.created" => format!("created  {}", format_sync_action(&self, server_id)),
			"mellow.server.syncing.action.updated" => format!("updated  {}", format_sync_action(&self, server_id)),
			"mellow.server.syncing.action.deleted" => format!("deleted  {}", format_sync_action(&self, server_id)),
			"mellow.server.syncing.settings.updated" => "updated the syncing settings".into(),
			"mellow.server.discord_logging.updated" => "updated the logging settings".into(),
			"mellow.server.ownership.changed" => "transferred ownership to {unimplemented}".into(),
			"mellow.server.automation.event.updated" => format!("updated the {} event", self.data.get("event_name").and_then(|x| x.as_str()).unwrap_or("unknown")),
			_ => self.kind.clone()
		}
	}
}

#[derive(Deserialize, Serialize)]
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

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "data")]
#[repr(u8)]
pub enum ServerLog {
	ActionLog(ActionLog) = 1 << 0,
	ServerProfileSync {
		#[serde(skip)]
		kind: ProfileSyncKind,
		member: DiscordMember,
		forced_by: Option<DiscordMember>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<UserConnection>
	} = 1 << 1,
	UserCompletedOnboarding {
		member: DiscordMember
	} = 1 << 2,
	EventResponseResult {
		invoker: DiscordMember,
		event_kind: String,
		member_result: EventResponseResultMemberResult
	} = 1 << 3
}

#[derive(Deserialize, Serialize)]
pub enum EventResponseResultMemberResult {
	None,
	Banned,
	Kicked
}

impl ServerLog {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
}

impl Server {
	pub async fn send_logs(&self, logs: Vec<ServerLog>) -> Result<()> {
		if logs.is_empty() {
			return Ok(());
		}
		
		if let Some(channel_id) = &self.logging_channel_id {
			let mut embeds: Vec<Embed> = vec![];
			for log in logs {
				let value = log.discriminant();
				if value == 4 || value == 8 || (self.logging_types & value) == value {
					match log {
						ServerLog::ActionLog(payload) => {
							let website_url = format!("https://hakumi.cafe/mellow/server/{}/settings/action_log", self.id);

							let mut details: Vec<String> = vec![];
							if payload.kind == "mellow.server.syncing.action.created" {
								if let Some(name) = payload.data.get("name").and_then(unwrap_string_or_array) {
									details.push(format!("* With name **{name}**"));
								}
								if let Some(requirements) = payload.data.get("requirements").and_then(|x| x.as_i64()) {
									details.push(format!("* With {requirements} requirement(s)"));
								}
							}
							if payload.kind == "mellow.server.syncing.action.created" || payload.kind == "mellow.server.syncing.action.updated" || payload.kind == "mellow.server.syncing.settings.updated" || payload.kind == "mellow.server.discord_logging.updated" || payload.kind == "mellow.server.automation.event.updated" {
								details.push(format!("* *View the full details [here]({website_url})*"));
							}

							embeds.push(Embed {
								author: Some(EmbedAuthor {
									url: Some(website_url),
									name: Some("New Action Log".into()),
									icon_url: payload.author.avatar_url.clone()
								}),
								description: Some(format!("### [{}](https://hakumi.cafe/user/{}) {}\n{}",
									payload.author.display_name(),
									payload.author.username,
									payload.action_string(&self.id),
									details.join("\n")
								)),
								..Default::default()
							});
						},
						ServerLog::ServerProfileSync { kind, member, forced_by, role_changes, nickname_change, relevant_connections } => {
							let mut fields: Vec<EmbedField> = vec![];
							if !role_changes.is_empty() {
								fields.push(EmbedField {
									name: "Role changes".into(),
									value: format!("```diff\n{}```", role_changes.iter().map(|x| match x.kind {
										RoleChangeKind::Added => format!("+ {}", x.display_name),
										RoleChangeKind::Removed => format!("- {}", x.display_name)
									}).collect::<Vec<String>>().join("\n")),
									inline: None
								});
							}
							if let Some(changes) = nickname_change {
								fields.push(EmbedField {
									name: "Nickname changes".into(),
									value: format!("```diff{}{}```",
										changes.0.map(|x| format!("\n- {x}")).unwrap_or("".into()),
										changes.1.map(|x| format!("\n+ {x}")).unwrap_or("".into())
									),
									inline: None
								});
							}
							if !relevant_connections.is_empty() {
								fields.push(EmbedField {
									name: "Relevant connections".into(),
									value: relevant_connections.iter().map(|x| x.display()).collect::<Vec<String>>().join("\n"),
									inline: None
								});
							}
	
							embeds.push(Embed {
								title: Some(match kind {
									ProfileSyncKind::Default => forced_by.and_then(|x| if x.id() == member.id() { None } else { Some(x) }).map_or_else(
										|| format!("{} synced their profile", member.display_name()),
										|x| format!("{} forcefully synced {}'s profile", x.display_name(), member.display_name())
									),
									ProfileSyncKind::NewMember => format!("{} joined and has been synced", member.display_name())
								}),
								author: Some(self.embed_author(&member, None)),
								fields: Some(fields),
								..Default::default()
							});
						},
						ServerLog::UserCompletedOnboarding { member } => {
							embeds.push(Embed {
								title: Some(format!("{} completed onboarding", member.display_name())),
								author: Some(self.embed_author(&member, None)),
								..Default::default()
							});
						},
						ServerLog::EventResponseResult { invoker, event_kind, member_result} => {
							embeds.push(Embed {
								title: Some(match member_result {
									EventResponseResultMemberResult::Banned => format!("{} was banned", invoker.display_name()),
									EventResponseResultMemberResult::Kicked => format!("{} was kicked", invoker.display_name()),
									_ => "no result".into()
								}),
								author: Some(self.embed_author(&invoker, Some(format!("Event Response Result ({event_kind})")))),
								..Default::default()
							});
						}
					}
				}
			}
	
			if !embeds.is_empty() {
				for chunk in embeds.chunks(10) {
					create_channel_message(channel_id, ChannelMessage {
						embeds: Some(chunk.to_vec()),
						..Default::default()
					}).await?;
				}
			}
		}

		Ok(())
	}

	fn embed_author(&self, member: &DiscordMember, title: Option<String>) -> EmbedAuthor {
		EmbedAuthor {
			url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", self.id, member.id())),
			name: title.or(member.user.global_name.clone()),
			icon_url: member.avatar.as_ref().or(member.user.avatar.as_ref()).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp?size=48", member.id())),
			..Default::default()
		}
	}
}