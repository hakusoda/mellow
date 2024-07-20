use mellow_cache::CACHE;
use mellow_models::hakumi::visual_scripting::{ DocumentKind, Variable };
use twilight_model::gateway::payload::incoming::MessageCreate;

use crate::{
	visual_scripting::process_document,
	Result
};

pub async fn message_create(message_create: MessageCreate) -> Result<()> {
	if !message_create.author.bot {
		if let Some(guild_id) = message_create.guild_id {
			if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MessageCreatedEvent).await? {
				if let Some(document) = document.clone_if_ready() {
					let variables = Variable::create_map([
						("member", Variable::from_partial_member(Some(&message_create.author), message_create.member.as_ref().unwrap(), &guild_id)),
						("message", (&message_create).into())
					], None);
					process_document(document, variables)
						.await
						.send_logs(guild_id)
						.await?;
				}
			}
		}
	}

	Ok(())
}