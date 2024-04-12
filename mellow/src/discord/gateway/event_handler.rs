use std::time::SystemTime;
use tokio::sync::RwLock;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	guild::VerificationLevel,
	gateway::payload::incoming::{ MemberAdd, MemberUpdate, MessageCreate }
};

use crate::{
	util::member_into_partial,
	model::{
		discord::DISCORD_MODELS,
		mellow::MELLOW_MODELS
	},
	traits::WithId,
	visual_scripting::{ Variable, DocumentKind },
	Result,
	PENDING_VERIFICATION_TIMER
};

static PENDING_MEMBERS: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>)>> = RwLock::const_new(vec![]);

pub async fn member_add(event_data: &MemberAdd) -> Result<()> {
	let user_id = event_data.user.id;
	let guild_id = event_data.guild_id;
	if event_data.member.pending {
		PENDING_MEMBERS.write().await.push((guild_id.clone(), user_id.clone()));
	}

	let document = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberJoinEvent).await?;
	if document.is_ready_for_stream() {
		let member = member_into_partial(event_data.member.clone()).with_id(user_id);
		let variables = Variable::create_map([
			("member", Variable::from_partial_member(Some(&event_data.user), &member, &guild_id))
		], None);
		document
			.process(variables)
			.await?
			.send_logs(guild_id)
			.await?;
	}

	Ok(())
}

pub async fn member_update(event_data: &MemberUpdate) -> Result<()> {
	if !event_data.pending {
		let key = (event_data.guild_id, event_data.user.id);
		let pending = &PENDING_MEMBERS;
		let mut pending = pending.write().await;
		if pending.contains(&key) {
			pending.retain(|x| *x != key);

			let guild_id = event_data.guild_id;
			if event_data.roles.is_empty() {
				let onboarding = DISCORD_MODELS.guild_onboarding(guild_id).await?;
				if !onboarding.enabled {
					let guild = DISCORD_MODELS.guild(guild_id).await?;
					match guild.verification_level {
						VerificationLevel::High => {
							PENDING_VERIFICATION_TIMER.write().await.push((guild_id.clone(), event_data.user.id.clone(), SystemTime::now()));
							tracing::info!("added {} to PENDING_VERIFICATION_TIMER", event_data.user.id);
							return Ok(());
						},
						_ => ()
					}
				}
			}

			let document = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await?;
			if document.is_ready_for_stream() {
				let variables = Variable::create_map([
					("member".into(), event_data.into())
				], None);
				document
					.process(variables)
					.await?
					.send_logs(guild_id)
					.await?;
			}
		}
	}

	Ok(())
}

pub async fn message_create(event_data: &MessageCreate) -> Result<()> {
	if let Some(guild_id) = event_data.guild_id {
		let document = MELLOW_MODELS.event_document(guild_id, DocumentKind::MessageCreatedEvent).await?;
		if document.is_ready_for_stream() {
			let variables = Variable::create_map([
				("member", Variable::from_partial_member(Some(&event_data.author), event_data.member.as_ref().unwrap(), &guild_id)),
				("message", event_data.into())
			], None);
			document
				.process(variables)
				.await?
				.send_logs(guild_id)
				.await?;
		}
	}

	Ok(())
}