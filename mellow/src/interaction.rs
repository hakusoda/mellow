use serde::{ Serialize, Deserialize };
use serde_repr::*;

use crate::{
	discord::DiscordMember,
	commands::COMMANDS,
	SlashResponse
};

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum ApplicationCommandKind {
	ChatInput = 1,
	User,
	Message
}

#[derive(Deserialize, Debug)]
pub struct ApplicationCommandData {
	pub id: String,
	pub name: String,
	#[serde(rename = "type")]
	pub kind: ApplicationCommandKind
}

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum InteractionKind {
	Ping = 1,
	ApplicationCommand,
	MessageComponent,
	ApplicationCommandAutocomplete,
	ModalSubmit
}

#[derive(Deserialize, Debug)]
pub struct InteractionPayload {
	#[serde(rename = "type")]
	pub kind: InteractionKind,
	pub data: Option<ApplicationCommandData>,
	pub token: String,
	pub member: Option<DiscordMember>,
	pub guild_id: Option<String>
}

#[derive(Serialize_repr, Debug)]
#[repr(u8)]
enum InteractionResponseKind {
	Pong = 1,
	ChannelMessageWithSource = 4,
	DeferredChannelMessageWithSource
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Embed {
	pub url: Option<String>,
	pub title: Option<String>,
	pub author: Option<EmbedAuthor>,
	pub fields: Option<Vec<EmbedField>>,
	pub description: Option<String>
}

impl Default for Embed {
	fn default() -> Self {
		Self {
			url: None,
			title: None,
			author: None,
			fields: None,
			description: None
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EmbedAuthor {
	pub url: Option<String>,
	pub name: Option<String>,
	pub icon_url: Option<String>
}

impl Default for EmbedAuthor {
	fn default() -> Self {
		Self {
			url: None,
			name: None,
			icon_url: None
		}
	}
}

#[derive(Clone, Serialize, Deserialize)]
pub struct EmbedField {
	pub name: String,
	pub value: String,
	pub inline: Option<bool>
}

#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub enum InteractionResponseData {
	ChannelMessageWithSource {
		flags: Option<u8>,
		embeds: Option<Vec<Embed>>,
		content: Option<String>
	},
	DeferredChannelMessageWithSource {
		flags: u8
	}
}

// i don't like this structure
#[derive(Serialize)]
struct InteractionResponse {
	#[serde(rename = "type")]
	kind: InteractionResponseKind,
	data: Option<InteractionResponseData>
}

pub async fn handle_request(body: String) -> impl actix_web::Responder {
	let payload: InteractionPayload = serde_json::from_str(&body).unwrap();
	match payload.kind {
		InteractionKind::ApplicationCommand => {
			if let Some(ref data) = payload.data {
				if let Some(command) = COMMANDS.iter().find(|x| x.name == data.name) {
					println!("executing {}", command.name);
					if let Some(callback) = command.slash_action {
						return match callback(payload).await {
							SlashResponse::Message { flags, content } =>
								actix_web::web::Json(InteractionResponse {
									kind: InteractionResponseKind::ChannelMessageWithSource,
									data: Some(InteractionResponseData::ChannelMessageWithSource {
										flags,
										embeds: None,
										content
									})
								}),
							SlashResponse::DeferMessage =>
								actix_web::web::Json(InteractionResponse {
									kind: InteractionResponseKind::DeferredChannelMessageWithSource,
									data: Some(InteractionResponseData::DeferredChannelMessageWithSource {
										flags: 64
									})
								})
						};
					}
				}
			}
			actix_web::web::Json(InteractionResponse {
				kind: InteractionResponseKind::ChannelMessageWithSource,
				data: Some(InteractionResponseData::ChannelMessageWithSource {
					flags: None,
					embeds: None,
					content: Some("PLACEHOLDER?!?!?!?".into())
				})
			})
		},
		_ => actix_web::web::Json(InteractionResponse {
			kind: InteractionResponseKind::Pong,
			data: None
		})
	}
}