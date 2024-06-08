use twilight_model::gateway::payload::incoming::InteractionCreate;

use crate::{ interaction::handle_interaction, Result, Context };

pub async fn interaction_create(context: Context, interaction_create: InteractionCreate) -> Result<()> {
	handle_interaction(context, interaction_create.0).await
}