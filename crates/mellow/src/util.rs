use dashmap::mapref::multiple::RefMulti;
use mellow_cache::CACHE;
use mellow_models::hakumi::user::ConnectionModel;
use mellow_util::hakuid::{
	marker::{ ConnectionMarker, UserMarker },
	HakuId
};
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::Result;

pub async fn user_server_connections(guild_id: Id<GuildMarker>, user_id: HakuId<UserMarker>) -> Result<Vec<RefMulti<'static, HakuId<ConnectionMarker>, ConnectionModel>>> {
	let user_connections = CACHE
		.mellow
		.user_settings(guild_id, user_id)
		.await?
		.user_connections();
	Ok(CACHE
		.hakumi
		.connections(&user_connections)
		.await?
	)
}