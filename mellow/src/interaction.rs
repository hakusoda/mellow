use hyper::{ body::Bytes, Request };
use serde::{ Serialize, Deserialize };
use serde_repr::*;
use ed25519_dalek::{ Verifier, Signature, VerifyingKey };
use http_body_util::{ combinators::BoxBody, BodyExt };

use crate::{
	discord::DiscordMember,
	commands::COMMANDS,
	http_service::{ json, empty },
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

pub async fn handle_request(request: Request<hyper::body::Incoming>) -> BoxBody<Bytes, hyper::Error> {
	let headers = request.headers().clone();
	let body = parse_body(
		String::from_utf8(request.collect().await.unwrap().to_bytes().to_vec()).unwrap(),
		headers["x-signature-ed25519"].to_str().unwrap(),
		headers["x-signature-timestamp"].to_str().unwrap()
	);
	let payload: InteractionPayload = serde_json::from_str(&body).unwrap();
	match payload.kind {
		InteractionKind::Ping => json(InteractionResponse {
			kind: InteractionResponseKind::Pong,
			data: None
		}),
		InteractionKind::ApplicationCommand => {
			if let Some(ref data) = payload.data {
				if let Some(command) = COMMANDS.iter().find(|x| x.name == data.name) {
					println!("executing {}", command.name);
					if let Some(callback) = command.slash_action {
						match callback(payload).await {
							SlashResponse::Message { flags, content } => {
								return json(InteractionResponse {
									kind: InteractionResponseKind::ChannelMessageWithSource,
									data: Some(InteractionResponseData::ChannelMessageWithSource {
										flags,
										embeds: None,
										content
									})
								});
							},
							SlashResponse::DeferMessage => return json(InteractionResponse {
								kind: InteractionResponseKind::DeferredChannelMessageWithSource,
								data: Some(InteractionResponseData::DeferredChannelMessageWithSource {
									flags: 64
								})
							})
						}
					}
				}
			}
			json(InteractionResponse {
				kind: InteractionResponseKind::ChannelMessageWithSource,
				data: Some(InteractionResponseData::ChannelMessageWithSource {
					flags: None,
					embeds: None,
					content: Some("PLACEHOLDER?!?!?!?".into())
				})
			})
		},
		_ => empty()
	}
}

fn parse_body(body: String, signature: &str, timestamp: &str) -> String {
	let public_key = hex::decode(std::env::var("DISCORD_PUBLIC_KEY").unwrap())
        .map(|vec| VerifyingKey::from_bytes(&vec.try_into().unwrap()).unwrap())
		.unwrap();
	public_key.verify(
        format!("{}{}", timestamp, body).as_bytes(),
        &hex::decode(&signature)
            .map(|vec| Signature::from_bytes(&vec.try_into().unwrap()))
			.unwrap()
    ).unwrap();
	body
}