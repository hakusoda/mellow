use chrono::{ Utc, DateTime };
use dashmap::mapref::one::Ref;
use mellow_cache::CACHE;
use mellow_models::{
	discord::guild::MemberModel,
	hakumi::visual_scripting::Variable
};
use mellow_util::DISCORD_INTERACTION_CLIENT;
use serde::{ Serialize, Deserialize };
use serde_repr::*;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker, ApplicationMarker, InteractionMarker },
		Id
	},
	http::interaction::{ InteractionResponse, InteractionResponseData, InteractionResponseType },
	guild::Permissions,
	channel::{ message::MessageFlags, Channel, Message },
	application::interaction::{ Interaction as TwilightInteraction, InteractionData, InteractionType }
};

use crate::{
	commands::COMMANDS,
	visual_scripting::{ process_document, variable_from_member },
	Result, Context, CommandResponse
};

#[derive(Clone, Debug, PartialEq)]
pub struct Interaction {
    pub app_permissions: Option<Permissions>,
    pub application_id: Id<ApplicationMarker>,
    pub channel: Option<Channel>,
    pub data: Option<InteractionData>,
    pub guild_id: Option<Id<GuildMarker>>,
    pub guild_locale: Option<String>,
    pub id: Id<InteractionMarker>,
    pub kind: InteractionType,
    pub locale: Option<String>,
    pub message: Option<Message>,
    pub token: String,
    pub user_id: Option<Id<UserMarker>>,
}

impl Interaction {
	/*pub async fn user(&self) -> Result<Option<Ref<'_, Id<UserMarker>, CachedUser>>> {
		Ok(if let Some(user_id) = self.user_id {
			Some(DISCORD_MODELS.user(user_id).await?)
		} else { None })
	}*/

	pub async fn member(&self) -> Result<Option<Ref<'static, (Id<GuildMarker>, Id<UserMarker>), MemberModel>>> {
		Ok(if let Some(user_id) = self.user_id && let Some(guild_id) = self.guild_id {
			Some(CACHE
				.discord
				.member(guild_id, user_id)
				.await?
			)
		} else { None })
	}
}

#[derive(Deserialize_repr, Debug)]
#[repr(u8)]
pub enum ApplicationCommandKind {
	ChatInput = 1,
	User,
	Message
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Embed {
	pub url: Option<String>,
	pub title: Option<String>,
	pub author: Option<EmbedAuthor>,
	pub fields: Option<Vec<EmbedField>>,
	pub footer: Option<EmbedFooter>,
	pub timestamp: Option<DateTime<Utc>>,
	pub description: Option<String>
}

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct EmbedAuthor {
	pub url: Option<String>,
	pub name: Option<String>,
	pub icon_url: Option<String>
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
			if let Some(guild_id) = data.guild_id && let Some(user_id) = interaction.user_id {
				/*let Some(command) = CACHE.mellow.command(data.id) else {
					return Ok(InteractionResponse {
						kind: InteractionResponseType::ChannelMessageWithSource,
						data: Some(InteractionResponseData {
							flags: Some(MessageFlags::EPHEMERAL),
							content: Some("<:niko_yawn:1226170445242568755> uhm... this custom command doesn't appear to exist anymore!`".into()),
							..Default::default()
						})
					});
				};*/
				let command = CACHE
					.mellow
					.command(data.id)
					.await?;
				let document = CACHE
					.hakumi
					.visual_scripting_document(command.document_id)
					.await?;
				if let Some(document) = document.clone_if_ready() {
					let token = interaction.token.clone();
					tokio::spawn(async move {
						let variables = Variable::create_map([
							("member", variable_from_member(guild_id, user_id).await?),
							("guild_id", guild_id.into()),
							("interaction_token", token.clone().into())
						], None);
						let action_tracker = process_document(document, variables)
							.await;
						if !action_tracker.replied {
							DISCORD_INTERACTION_CLIENT
								.update_response(&token)
								.content(Some("oh dear, something went wrong while processing this custom command!"))
								.await?;
						}
						
						action_tracker
							.send_logs(guild_id)
							.await?;
						Ok::<(), crate::error::Error>(())
					});
					
					Ok(InteractionResponse {
						kind: InteractionResponseType::DeferredChannelMessageWithSource,
						data: if command.is_ephemeral { Some(InteractionResponseData {
							flags: Some(MessageFlags::EPHEMERAL),
							..Default::default()
						}) } else { None }
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
				let response = match (command.handler)(context, interaction).await {
					Ok(x) => x,
					Err(error) => {
						println!("{error}");
						return Err(error);
					}
				};
				Ok(match response {
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
						flags: Some(MessageFlags::EPHEMERAL),
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
pub async fn handle_interaction(context: Context, interaction: TwilightInteraction) -> Result<()> {
	let id = interaction.id;
	let token = interaction.token.clone();
	if let Some(user) = interaction.author() {
		CACHE
			.discord
			.users
			.insert(user.id, user.clone().into());
	}

	let interaction = Interaction {
		app_permissions: interaction.app_permissions,
		application_id: interaction.application_id,
		channel: interaction.channel,
		data: interaction.data,
		guild_id: interaction.guild_id,
		guild_locale: interaction.guild_locale,
		id: interaction.id,
		kind: interaction.kind,
		locale: interaction.locale,
		message: interaction.message,
		token: interaction.token,
		user_id: match interaction.member {
			Some(member) => member.user.map(|x| x.id),
			None => interaction.user.map(|x| x.id)
		}
	};

	let response = parse_interaction(context, interaction)
		.await?;
	DISCORD_INTERACTION_CLIENT
		.create_response(id, &token, &response)
		.await?;

	Ok(())
}