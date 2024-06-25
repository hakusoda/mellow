use dashmap::{
	mapref::one::Ref,
	DashMap
};
use once_cell::sync::Lazy;
use twilight_model::id::{
	marker::{ UserMarker, GuildMarker },
	Id
};

use user::User;
use crate::{
	database::DATABASE,
	visual_scripting::Document,
	Result
};

pub mod id;
pub mod user;

pub use id::{
	marker::{ UserMarker as HakuUserMarker, DocumentMarker },
	HakuId
};
pub static HAKUMI_MODELS: Lazy<HakumiModels> = Lazy::new(HakumiModels::default);

#[derive(Debug, Default)]
pub struct HakumiModels {
	pub users: DashMap<HakuId<HakuUserMarker>, User>,
	//user_connections: DashMap<(HakuId<HakuUserMarker>, HakuId<ConnectionMarker>), Connection>,
	pub users_by_discord: DashMap<(Id<GuildMarker>, Id<UserMarker>), HakuId<HakuUserMarker>>,

	pub vs_documents: DashMap<HakuId<DocumentMarker>, Document>
}

impl HakumiModels {
	pub async fn user(&self, user_id: HakuId<HakuUserMarker>) -> Result<Ref<'_, HakuId<HakuUserMarker>, User>> {
		Ok(if let Some(item) = self.users.get(&user_id) {
			tracing::info!("users.read (user_id={user_id})");
			item
		} else {
			let mut new_item: User = DATABASE.from("users")
				.select("id, server_settings:mellow_user_server_settings(user_connections), connections:user_connections( id, sub, type, username, display_name, oauth_authorisations:user_connection_oauth_authorisations( id, token_type, expires_at, access_token, refresh_token ) )")
				.eq("id", user_id.to_string())
				.limit(1)
				.single()
				.await?
				.value;
			tracing::info!("users.write (user_id={user_id})");

			new_item.refresh_oauth().await?;
			
			self.users
				.entry(user_id)
				.insert(new_item)
				.downgrade()
		})
	}

	pub async fn user_by_discord(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Option<Ref<'_, (Id<GuildMarker>, Id<UserMarker>), HakuId<HakuUserMarker>>>> {
		let key = (guild_id, user_id);
		Ok(if let Some(item) = self.users_by_discord.get(&key) {
			tracing::info!("users_by_discord.read (guild_id={guild_id}) (discord_id={user_id})");
			Some(item)
		} else {
			let mut new_item = match get_user_reference(user_id).await? {
				Some(x) => x,
				None => return Ok(None)
			};
			new_item.refresh_oauth().await?;

			let user_id: HakuId<HakuUserMarker> = new_item.id;
			tracing::info!("users.write (user_id={user_id})");
			tracing::info!("users_by_discord.write (guild_id={guild_id}) (discord_id={user_id}) (user_id={user_id})");

			self.users.insert(user_id, new_item);
			Some(self.users_by_discord
				.entry(key)
				.insert(user_id)
				.downgrade()
			)
		})
	}

	pub async fn vs_document(&self, document_id: HakuId<DocumentMarker>) -> Result<Ref<'_, HakuId<DocumentMarker>, Document>> {
		Ok(if let Some(item) = self.vs_documents.get(&document_id) {
			tracing::debug!("vs_documents.read (document_id={document_id})");
			item
		} else {
			let new_item: Document = DATABASE.from("visual_scripting_documents")
				.select("id,name,kind,active,definition")
				.eq("id", document_id.to_string())
				.limit(1)
				.single()
				.await?
				.value;
			tracing::debug!("vs_documents.write (document_id={document_id})",);
			
			self.vs_documents
				.entry(document_id)
				.insert(new_item)
				.downgrade()
		})
	}
}

async fn get_user_reference(user_id: Id<UserMarker>) -> Result<Option<User>> {
	Ok(DATABASE.from("user_connections")
		.select("...users( id, connections:user_connections( id, sub, type, username, display_name, oauth_authorisations:user_connection_oauth_authorisations( id, token_type, expires_at, access_token, refresh_token ) ) )")
		.eq("sub", user_id.to_string())
		.limit(1)
		.maybe_single()
		.await?
		.value
	)
}