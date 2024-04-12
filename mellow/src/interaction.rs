use serde::{ Serialize, Deserialize };
use chrono::{ Utc, DateTime };
use serde_repr::*;
use twilight_model::{
	http::interaction::{ InteractionResponse, InteractionResponseData, InteractionResponseType },
	channel::message::MessageFlags,
	application::interaction::{ Interaction, InteractionData }
};

use crate::{
	discord::INTERACTION,
	commands::COMMANDS,
	database::ServerCommand,
	visual_scripting::Variable,
	Result, Context, CommandResponse
};

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum ApplicationCommandKind {
	ChatInput = 1,
	User,
	Message
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Embed {
	pub url: Option<String>,
	pub title: Option<String>,
	pub author: Option<EmbedAuthor>,
	pub fields: Option<Vec<EmbedField>>,
	pub footer: Option<EmbedFooter>,
	pub timestamp: Option<DateTime<Utc>>,
	pub description: Option<String>
}

impl Default for Embed {
	fn default() -> Self {
		Self {
			url: None,
			title: None,
			author: None,
			fields: None,
			footer: None,
			timestamp: None,
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

#[derive(Clone, Serialize, Deserialize)]
pub struct EmbedFooter {
	pub text: String,
	pub icon_url: Option<String>
}

async fn parse_interaction(context: Context, interaction: Interaction) -> Result<InteractionResponse> {
	match interaction.data.as_ref().unwrap() {
		InteractionData::ApplicationCommand(data) => {
			if let Some(guild_id) = data.guild_id {
				let command = match ServerCommand::fetch(&guild_id, data.name.clone()).await {
					Ok(x) => x,
					Err(x) => {
						tracing::error!("error while fetching server command (guild_id={}) (name={}) {}", guild_id, data.name, x);
						return Ok(InteractionResponse {
							kind: InteractionResponseType::ChannelMessageWithSource,
							data: Some(InteractionResponseData {
								flags: Some(MessageFlags::EPHEMERAL),
								content: Some(format!("<:niko_yawn:1226170445242568755> an unexpected error occurred while fetching the information for this command... oopsie daisy!\n```sh\n{x}```")),
								..Default::default()
							})
						})
					}
				};
				if command.document.is_ready_for_stream() {
					let user = interaction.user.clone();
					let token = interaction.token.clone();
					let member = interaction.member.clone().unwrap();
					let guild_id = guild_id.clone();
					tokio::spawn(async move {
						let variables = Variable::create_map([
							("member", Variable::from_partial_member(user.as_ref(), &member, &guild_id)),
							("guild_id", guild_id.into()),
							("interaction_token", token.into())
						], None);
						command.document
							.process(variables)
							.await?
							.send_logs(guild_id)
							.await?;
						Ok::<(), crate::error::Error>(())
					});
					
					Ok(InteractionResponse {
						kind: InteractionResponseType::DeferredChannelMessageWithSource,
						data: Some(InteractionResponseData {
							flags: Some(MessageFlags::EPHEMERAL),
							..Default::default()
						})
					})
				} else {
					Ok(InteractionResponse {
						kind: InteractionResponseType::ChannelMessageWithSource,
						data: Some(InteractionResponseData {
							flags: Some(MessageFlags::EPHEMERAL),
							content: Some("<:niko_yawn:1226170445242568755> this custom command currently does absolutely nothing... go tell a server admin about it!!!".into()),
							..Default::default()
						})
					})
				}
			} else if let Some(command) = COMMANDS.iter().find(|x| x.name == data.name) {
				Ok(match (command.handler)(context, interaction).await.map_err(|x| { println!("{x}"); x })? {
					CommandResponse::Message { flags, content } =>
						InteractionResponse {
							kind: InteractionResponseType::ChannelMessageWithSource,
							data: Some(InteractionResponseData {
								flags,
								content,
								..Default::default()
							})
						},
					CommandResponse::Defer =>
						InteractionResponse {
							kind: InteractionResponseType::DeferredChannelMessageWithSource,
							data: Some(InteractionResponseData {
								flags: Some(MessageFlags::EPHEMERAL),
								..Default::default()
							})
						}
				})
			} else {
				Ok(InteractionResponse {
					kind: InteractionResponseType::ChannelMessageWithSource,
					data: Some(InteractionResponseData {
						content: Some("<:niko_look_left:1227198516590411826> erm... this command hasn't been implemented yet...".into()),
						..Default::default()
					})
				})
			}
		},
		_ => unimplemented!()
	}
}

#[tracing::instrument(skip(context), level = "trace")]
pub async fn handle_interaction(context: Context, interaction: Interaction) -> Result<()> {
	let id = interaction.id.clone();
	let token = interaction.token.clone();
	let response = parse_interaction(context, interaction).await?;
	INTERACTION.create_response(id, &token, &response).await?;

	Ok(())
}