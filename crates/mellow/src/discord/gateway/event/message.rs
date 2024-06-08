use twilight_model::gateway::payload::incoming::MessageCreate;

use crate::{
	model::mellow::MELLOW_MODELS,
	visual_scripting::{ Variable, DocumentKind },
	Result
};

pub async fn message_create(message_create: MessageCreate) -> Result<()> {
	if !message_create.author.bot {
		if let Some(guild_id) = message_create.guild_id {
			if let Some(document) = MELLOW_MODELS.event_document(guild_id, DocumentKind::MessageCreatedEvent).await? {
				if document.is_ready_for_stream() {
					let variables = Variable::create_map([
						("member", Variable::from_partial_member(Some(&message_create.author), message_create.member.as_ref().unwrap(), &guild_id)),
						("message", (&message_create).into())
					], None);
					document
						.process(variables)
						.await?
						.send_logs(guild_id)
						.await?;
				}
			}
		}
	}

	Ok(())
}