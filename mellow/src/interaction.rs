use serde::{ Serialize, Deserialize };
use actix_web::web::Json;
use serde_repr::*;
use futures_util::StreamExt;
use twilight_model::application::interaction::{ Interaction, InteractionData, InteractionType };

use crate::{
	http::{ ApiError, ApiResult },
	server::Server,
	discord::edit_original_response,
	commands::COMMANDS,
	database::ServerCommand,
	visual_scripting::{ Variable, ElementKind },
	SlashResponse
};

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum ApplicationCommandKind {
	ChatInput = 1,
	User,
	Message
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
pub struct InteractionResponse {
	#[serde(rename = "type")]
	kind: InteractionResponseKind,
	data: Option<InteractionResponseData>
}

pub async fn handle_request(body: String) -> ApiResult<Json<InteractionResponse>> {
	let payload: Interaction = serde_json::from_str(&body).unwrap();
	match payload.kind {
		InteractionType::Ping => Ok(Json(InteractionResponse {
			kind: InteractionResponseKind::Pong,
			data: None
		})),
		_ => match payload.data.as_ref().unwrap() {
			InteractionData::ApplicationCommand(data) => {
				if let Some(guild_id) = data.guild_id {
					let command = ServerCommand::fetch(&guild_id, data.name.clone()).await?;
					let token = payload.token.clone();
					let member = payload.member.clone().unwrap();
					let guild_id = guild_id.clone();
					tokio::spawn(async move {
						let (mut stream, tracker) = command.document.into_stream(Variable::create_map([
							("member".into(), Variable::from_partial_member(payload.user.as_ref(), &member, &guild_id))
						], None));
						while let Some((element, variables)) = stream.next().await {
							match element.kind {
								ElementKind::GetLinkedPatreonCampaign => {
									let server = Server::fetch(&guild_id).await?;
									variables.write().await.set("campaign", crate::patreon::get_campaign(server.oauth_authorisations.first().unwrap()).await?.into());
								},
								ElementKind::InteractionReply(data) =>
									edit_original_response(&token, InteractionResponseData::ChannelMessageWithSource {
										flags: None,
										embeds: None,
										content: Some(data.resolve(&*variables.read().await))
									}).await?,
							_ => ()
							}
						}
						tracker.send_logs(&guild_id).await?;
						Ok::<(), crate::error::Error>(())
					});
					
					return Ok(Json(InteractionResponse {
						kind: InteractionResponseKind::DeferredChannelMessageWithSource,
						data: Some(InteractionResponseData::DeferredChannelMessageWithSource {
							flags: 64
						})
					}));
				} else {
					if let Some(command) = COMMANDS.iter().find(|x| x.name == data.name) {
						if let Some(callback) = command.slash_action {
							return Ok(Json(match callback(payload).await.map_err(|x| { println!("{x}"); x })? {
								SlashResponse::Message { flags, content } =>
									InteractionResponse {
										kind: InteractionResponseKind::ChannelMessageWithSource,
										data: Some(InteractionResponseData::ChannelMessageWithSource {
											flags,
											embeds: None,
											content
										})
									},
								SlashResponse::DeferMessage =>
									InteractionResponse {
										kind: InteractionResponseKind::DeferredChannelMessageWithSource,
										data: Some(InteractionResponseData::DeferredChannelMessageWithSource {
											flags: 64
										})
									}
							}));
						}
					}
				}
				Ok(Json(InteractionResponse {
					kind: InteractionResponseKind::ChannelMessageWithSource,
					data: Some(InteractionResponseData::ChannelMessageWithSource {
						flags: None,
						embeds: None,
						content: Some("PLACEHOLDER?!?!?!?".into())
					})
				}))
			},
			_ => Err(ApiError::NotImplemented)
		}
	}
}