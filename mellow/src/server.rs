use serde::{ Serialize, Deserialize };

use crate::{
	http::routes::ActionLogWebhookPayload,
	discord::{ DiscordMember, ChannelMessage, create_channel_message },
	syncing::{ RoleChange, NicknameChange, RoleChangeKind },
	database::{ Server, UserConnection },
	interaction::{ Embed, EmbedField, EmbedAuthor }
};

#[derive(Deserialize, Serialize)]
#[serde(tag = "type", content = "data")]
#[repr(u8)]
pub enum ServerLog {
	ActionLog(ActionLogWebhookPayload) = 1 << 0,
	ServerProfileSync {
		member: DiscordMember,
		forced_by: Option<DiscordMember>,
		role_changes: Vec<RoleChange>,
		nickname_change: Option<NicknameChange>,
		relevant_connections: Vec<UserConnection>
	} = 1 << 1
}

impl ServerLog {
    fn discriminant(&self) -> u8 {
        unsafe { *(self as *const Self as *const u8) }
    }
}

impl Server {
	pub async fn send_logs(&self, logs: Vec<ServerLog>) {
		if let Some(channel_id) = &self.logging_channel_id {
			let mut embeds: Vec<Embed> = vec![];
			for log in logs {
				let value = log.discriminant();
				if (self.logging_types & value) == value {
					match log {
						ServerLog::ActionLog(payload) => {
							embeds.push(Embed {
								title: Some(
									format!("{} {}",
										payload.author.display_name(),
										match payload.kind.as_str() {
											"mellow.server.api_key.created" => "created a new API Key",
											"mellow.server.syncing.action.created" => "created [AN ACTION]",
											"mellow.server.syncing.action.updated" => "updated [AN ACTION]",
											"mellow.server.syncing.action.deleted" => "deleted [AN ACTION]",
											"mellow.server.syncing.settings.updated" => "updated the syncing settings",
											"mellow.server.discord_logging.updated" => "updated Discord logging settings",
											_ => &payload.kind
										}
									)
								),
								..Default::default()
							});
						},
						ServerLog::ServerProfileSync { member, forced_by, role_changes, nickname_change, relevant_connections } => {
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
								title: Some(forced_by.and_then(|x| if x.user.id == member.user.id { None } else { Some(x) }).map_or_else(
									|| format!("{} synced their profile", member.display_name()),
									|x| format!("{} forcefully synced {}'s profile", x.display_name(), member.display_name())
								)),
								author: Some(EmbedAuthor {
									url: Some(format!("https://hakumi.cafe/mellow/server/{}/member/{}", self.id, member.user.id)),
									name: member.user.global_name,
									icon_url: member.avatar.or(member.user.avatar).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp?size=48", member.user.id)),
									..Default::default()
								}),
								fields: Some(fields),
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
					}).await;
				}
			}
		}
	}
}