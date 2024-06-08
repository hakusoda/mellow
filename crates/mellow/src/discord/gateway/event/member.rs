use std::time::SystemTime;
use tokio::sync::RwLock;
use twilight_model::{
	id::{
		marker::{ UserMarker, GuildMarker },
		Id
	},
	guild::VerificationLevel,
	gateway::payload::incoming::{ MemberAdd, MemberChunk, MemberUpdate, MemberRemove }
};

use crate::{
	visual_scripting::{ Variable, DocumentKind },
	Result, Context,
	DISCORD_MODELS, MELLOW_MODELS, PENDING_VERIFICATION_TIMER
};

static PENDING_MEMBERS: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>)>> = RwLock::const_new(vec![]);

pub async fn member_add(member_add: MemberAdd) -> Result<()> {
	let user_id = member_add.user.id;
	let guild_id = member_add.guild_id;
	DISCORD_MODELS.members.insert((guild_id, user_id), member_add.member.clone().into());
	tracing::info!("model.discord.member.create (guild_id={guild_id}) (user_id={user_id})");

	if member_add.member.pending {
		PENDING_MEMBERS.write().await.push((guild_id, user_id));
	}

	if let Some(document) = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberJoinEvent).await? {
		if document.is_ready_for_stream() {
			let member = DISCORD_MODELS.member(guild_id, user_id).await?;
			let variables = Variable::create_map([
				("member", Variable::from_member(member.value(), guild_id).await?)
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

pub async fn member_chunk(context: Context, member_chunk: MemberChunk) -> Result<()> {
	for member in member_chunk.members {
		DISCORD_MODELS.members.insert((member_chunk.guild_id, member.user.id), member.into());
	}
	if member_chunk.chunk_index == member_chunk.chunk_count - 1 && let Some(nonce) = member_chunk.nonce.and_then(|x| x.parse().ok()) {
		if let Some(value) = context.member_requests.remove(&nonce) {
			value.1.send(()).unwrap();
			*context.member_request_index.lock().await -= 1;
		}
	}

	Ok(())
}

pub async fn member_update(member_update: MemberUpdate) -> Result<()> {
	let user_id = member_update.user.id;
	let guild_id = member_update.guild_id;
	if !member_update.pending {
		let key = (guild_id, user_id);
		let pending = &PENDING_MEMBERS;
		let mut pending = pending.write().await;
		if pending.contains(&key) {
			pending.retain(|x| *x != key);
			
			if member_update.roles.is_empty() {
				let onboarding = DISCORD_MODELS.guild_onboarding(guild_id).await?;
				if !onboarding.enabled {
					let guild = DISCORD_MODELS.guild(guild_id).await?;
					if guild.verification_level == VerificationLevel::High {
						PENDING_VERIFICATION_TIMER.write().await.push((guild_id, user_id, SystemTime::now()));
						tracing::info!("added {} to PENDING_VERIFICATION_TIMER", user_id);

						return Ok(());
					}
				}
			}

			if let Some(document) = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await? {
				if document.is_ready_for_stream() {
					let variables = Variable::create_map([
						("member", (&member_update).into())
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

	if let Some(document) = MELLOW_MODELS.event_document(guild_id, DocumentKind::MemberUpdatedEvent).await? {
		if document.is_ready_for_stream() {
			let variables = Variable::create_map([
				("old_member", match DISCORD_MODELS.members.get(&(guild_id, user_id)) {
					Some(x) => Variable::from_member(x.value(), guild_id).await?,
					None => (&member_update).into()
				}),
				("new_member", (&member_update).into())
			], None);
			document
				.process(variables)
				.await?
				.send_logs(guild_id)
				.await?;
		}
	}

	if let Some(mut member) = DISCORD_MODELS.members.get_mut(&(member_update.guild_id, member_update.user.id)) {
		member.update(&member_update);
	}

	Ok(())
}

pub async fn member_remove(member_remove: MemberRemove) -> Result<()> {
	let user_id = member_remove.user.id;
	let guild_id = member_remove.guild_id;
	DISCORD_MODELS.members.remove(&(guild_id, user_id));
	tracing::info!("model.discord.member.delete (guild_id={guild_id}) (user_id={user_id})");

	Ok(())
}