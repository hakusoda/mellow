use uuid::Uuid;
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

pub mod user;

pub static HAKUMI_MODELS: Lazy<HakumiModels> = Lazy::new(HakumiModels::default);

#[derive(Debug, Default)]
pub struct HakumiModels {
	users: DashMap<Uuid, User>,
	users_by_discord: DashMap<(Id<GuildMarker>, Id<UserMarker>), Uuid>,

	pub vs_documents: DashMap<Uuid, Document>
}

impl HakumiModels {
	pub async fn user(&self, user_id: &Uuid) -> Result<Ref<'_, Uuid, User>> {
		Ok(if let Some(item) = self.users.get(&user_id) {
			tracing::info!("users.read (user_id={user_id})");
			item
		} else {
			let new_item: User = simd_json::from_slice(&mut DATABASE.from("users")
				.select("id,server_settings:mellow_user_server_settings(user_connections),connections:user_connections(id,sub,type,username,display_name,oauth_authorisations:user_connection_oauth_authorisations(token_type,expires_at,access_token,refresh_token))")
				.eq("id", user_id.to_string())
				.limit(1)
				.single()
				.execute()
				.await?
				.bytes()
				.await?
				.to_vec()
			)?;
			tracing::info!("users.write (user_id={user_id})");
			
			self.users.insert(user_id.clone(), new_item);
			self.users.get(&user_id).unwrap()
		})
	}

	pub async fn user_by_discord(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Option<Ref<'_, Uuid, User>>> {
		let key = (guild_id, user_id);
		Ok(if let Some(item) = self.users_by_discord.get(&key) {
			tracing::info!("users_by_discord.read (guild_id={guild_id}) (discord_id={user_id})");
			Some(self.user(item.value()).await?)
		} else {
			let new_item = get_user_reference(guild_id, user_id).await?;
			let uuid = new_item.id.clone();
			tracing::info!("users.write (user_id={uuid})");
			tracing::info!("users_by_discord.write (guild_id={guild_id}) (discord_id={user_id}) (user_id={uuid})");

			self.users.insert(uuid.clone(), new_item);
			self.users_by_discord.insert(key, uuid.clone());
			self.users.get(&uuid)
		})
	}

	pub async fn vs_document(&self, document_id: &Uuid) -> Result<Ref<'_, Uuid, Document>> {
		Ok(if let Some(item) = self.vs_documents.get(&document_id) {
			tracing::debug!("vs_documents.read (document_id={document_id})");
			item
		} else {
			let new_item: Document = simd_json::from_slice(&mut DATABASE.from("visual_scripting_documents")
				.select("id,name,kind,active,definition")
				.eq("id", document_id.to_string())
				.limit(1)
				.single()
				.execute()
				.await?
				.bytes()
				.await?
				.to_vec()
			)?;
			tracing::debug!("vs_documents.write (document_id={document_id})",);
			
			self.vs_documents.insert(document_id.clone(), new_item);
			self.vs_documents.get(&document_id).unwrap()
		})
	}
}

async fn get_user_reference(guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<User> {
	Ok(simd_json::from_slice(&mut DATABASE.from("user_connections")
		.select("...users(id,server_settings:mellow_user_server_settings(user_connections),connections:user_connections(id,sub,type,username,display_name,oauth_authorisations:user_connection_oauth_authorisations(token_type,expires_at,access_token,refresh_token)))")
		.eq("sub", user_id.to_string())
		.eq("users.mellow_user_server_settings.server_id", guild_id.to_string())
		.limit(1)
		.single()
		.execute()
		.await?
		.bytes()
		.await?
		.to_vec()
	)?)
}