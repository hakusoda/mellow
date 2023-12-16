use serde::{ Serialize, Deserialize };

use crate::{
	discord::{ DiscordMember, ChannelMessage, create_channel_message },
	syncing::{ RoleChange, NicknameChange, RoleChangeKind },
	database::{ Server, UserConnection },
	interaction::{ Embed, EmbedField, EmbedAuthor }
};

#[derive(Debug)]
pub struct Log {
	pub kind: LogKind,
	pub data: serde_json::Value
}

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum LogKind {
	#[allow(dead_code)]
	AuditLog = 1 << 0,
	ServerProfileSync = 1 << 1
}

#[derive(Serialize, Deserialize)]
struct ServerProfileSyncLog {
	member: DiscordMember,
	role_changes: Vec<RoleChange>,
	nickname_change: Option<NicknameChange>,
	relevant_connections: Vec<UserConnection>
}

pub async fn send_logs(server: &Server, logs: Vec<Log>) {
	if let Some(channel_id) = &server.logging_channel_id {
		let mut embeds: Vec<Embed> = vec![];
		for log in logs {
			if (server.logging_types & log.kind.clone() as u8) == log.kind.clone() as u8 {
				match log.kind {
					LogKind::AuditLog => {

					},
					LogKind::ServerProfileSync => {
						let data: ServerProfileSyncLog = serde_json::from_value(log.data).unwrap();
						let mut fields: Vec<EmbedField> = vec![];
						if !data.role_changes.is_empty() {
							fields.push(EmbedField {
								name: "Role changes".into(),
								value: format!("```diff\n{}```", data.role_changes.iter().map(|x| match x.kind {
									RoleChangeKind::Added => format!("+ {}", x.display_name),
									RoleChangeKind::Removed => format!("- {}", x.display_name)
								}).collect::<Vec<String>>().join("\n")),
								inline: None
							});
						}
						if let Some(changes) = data.nickname_change {
							fields.push(EmbedField {
								name: "Nickname changes".into(),
								value: format!("```diff{}{}```",
									changes.0.map(|x| format!("\n- {x}")).unwrap_or("".into()),
									changes.1.map(|x| format!("\n+ {x}")).unwrap_or("".into())
								),
								inline: None
							});
						}
						if !data.relevant_connections.is_empty() {
							fields.push(EmbedField {
								name: "Relevant connections".into(),
								value: data.relevant_connections.iter().map(|x| x.display()).collect::<Vec<String>>().join("\n"),
								inline: None
							});
						}

						embeds.push(Embed {
							title: Some(format!("{} synced their profile", data.member.user.global_name.clone().unwrap_or(data.member.user.username))),
							author: Some(EmbedAuthor {
								name: data.member.user.global_name,
								icon_url: data.member.avatar.or(data.member.user.avatar).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp?size=48", data.member.user.id)),
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