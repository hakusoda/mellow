use twilight_gateway::Event;

use crate::{ Result, Context };

pub mod guild;
pub mod interaction;
pub mod member;
pub mod message;
pub mod misc;
pub mod role;

pub fn handle_event(context: &Context, event: Event) {
	let event_kind = event.kind();
	tracing::info!("handle_event {event_kind:?}");

	if let Err(error) = match event {
		Event::GuildCreate(x) => guild::guild_create(*x),
		Event::GuildUpdate(x) => guild::guild_update(*x),
		Event::GuildDelete(x) => guild::guild_delete(x),
		Event::InteractionCreate(x) => spawn(interaction::interaction_create(context.clone(), *x)),
		Event::MemberAdd(x) => spawn(member::member_add(*x)),
		Event::MemberChunk(x) => spawn(member::member_chunk(context.clone(), x)),
		Event::MemberUpdate(x) => spawn(member::member_update(*x)),
		Event::MemberRemove(x) => spawn(member::member_remove(x)),
		Event::MessageCreate(x) => spawn(message::message_create(*x)),
		Event::Ready(x) => spawn(misc::ready(*x)),
		Event::RoleCreate(x) => role::role_create(x),
		Event::RoleUpdate(x) => role::role_update(x),
		Event::RoleDelete(x) => role::role_delete(x),
		_ => Ok(())
	} {
		println!("error occurred in event handler! {error}");
	}
}

fn spawn<F: Future<Output = Result<()>> + Send + 'static>(future: F) -> Result<()> {
	tokio::spawn(async move {
		if let Err(error) = future.await {
			println!("error occurred in async event handler! {error}");
		}
	});

	Ok(())
}