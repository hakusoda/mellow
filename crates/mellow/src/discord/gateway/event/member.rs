use mellow_cache::CACHE;
use mellow_models::hakumi::visual_scripting::{ DocumentKind, Variable };
use std::time::SystemTime;
use tokio::sync::RwLock;
use twilight_model::{
	gateway::payload::incoming::{ MemberAdd, MemberChunk, MemberRemove, MemberUpdate },
	guild::VerificationLevel,
	id::{
		marker::{ GuildMarker, UserMarker },
		Id
	}
};

use crate::{
	visual_scripting::{ process_document, variable_from_member },
	Result, Context,
	PENDING_VERIFICATION_TIMER
};

static PENDING_MEMBERS: RwLock<Vec<(Id<GuildMarker>, Id<UserMarker>)>> = RwLock::const_new(vec![]);

pub async fn member_add(member_add: MemberAdd) -> Result<()> {
	let user_id = member_add.user.id;
	let guild_id = member_add.guild_id;
	let is_pending = member_add.pending;
	CACHE
		.discord
		.members
		.insert((guild_id, user_id), member_add.member.into());
	tracing::info!("model.discord.member.create (guild_id={guild_id}) (user_id={user_id})");

	if is_pending {
		PENDING_MEMBERS.write().await.push((guild_id, user_id));
	}

	if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MemberJoinEvent).await? {
		if let Some(document) = document.clone_if_ready() {
			let variables = Variable::create_map([
				("member", variable_from_member(guild_id, user_id).await?)
			], None);
			process_document(document, variables)
				.await
				.send_logs(guild_id)
				.await?;
		}
	}

	Ok(())
}

pub async fn member_chunk(context: Context, member_chunk: MemberChunk) -> Result<()> {
	for member in member_chunk.members {
		CACHE.discord.members.insert((member_chunk.guild_id, member.user.id), member.into());
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
				let onboarding = CACHE.discord.guild_onboarding(guild_id).await?;
				if !onboarding.enabled {
					let guild = CACHE.discord.guild(guild_id).await?;
					if guild.verification_level == VerificationLevel::High {
						PENDING_VERIFICATION_TIMER.write().await.push((guild_id, user_id, SystemTime::now()));
						tracing::info!("added {} to PENDING_VERIFICATION_TIMER", user_id);

						return Ok(());
					}
				}
			}

			if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MemberCompletedOnboardingEvent).await? {
				if let Some(document) = document.clone_if_ready() {
					let variables = Variable::create_map([
						("member", (&member_update).into())
					], None);
					process_document(document, variables)
						.await
						.send_logs(guild_id)
						.await?;
				}
			}
		}
	}

	if let Some(document) = CACHE.mellow.event_document(guild_id, DocumentKind::MemberUpdatedEvent).await? {
		if let Some(document) = document.clone_if_ready() {
			let variables = Variable::create_map([
				("old_member", match CACHE.discord.members.contains_key(&(guild_id, user_id)) {
					true => variable_from_member(guild_id, user_id).await?,
					false => (&member_update).into()
				}),
				("new_member", (&member_update).into())
			], None);
			process_document(document, variables)
				.await
				.send_logs(guild_id)
				.await?;
		}
	}

	if let Some(mut member) = CACHE.discord.members.get_mut(&(member_update.guild_id, member_update.user.id)) {
		member.update(&member_update);
	}

	Ok(())
}

pub async fn member_remove(member_remove: MemberRemove) -> Result<()> {
	let user_id = member_remove.user.id;
	let guild_id = member_remove.guild_id;
	CACHE.discord.members.remove(&(guild_id, user_id));
	tracing::info!("model.discord.member.delete (guild_id={guild_id}) (user_id={user_id})");

	Ok(())
}