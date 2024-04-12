use uuid::Uuid;
use dashmap::{
	mapref::one::Ref,
	DashMap
};
use once_cell::sync::Lazy;
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	model::hakumi::HAKUMI_MODELS,
	database::DATABASE,
	visual_scripting::{ Document, DocumentKind },
	Result
};
use server::Server;

pub mod server;

pub static MELLOW_MODELS: Lazy<MellowModels> = Lazy::new(MellowModels::default);

#[derive(Debug, Default)]
pub struct MellowModels {
	pub servers: DashMap<Id<GuildMarker>, Server>,
	event_documents: DashMap<(Id<GuildMarker>, DocumentKind), Uuid>
}

impl MellowModels {
	pub async fn server(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, Server>> {
		Ok(if let Some(item) = self.servers.get(&guild_id) {
			tracing::debug!("servers.read (guild_id={guild_id})");
			item
		} else {
			let new_item: Server = simd_json::from_slice(&mut DATABASE.from("mellow_servers")
				.select("id,default_nickname,allow_forced_syncing,logging_types,logging_channel_id,actions:mellow_binds(id,name,type,metadata,requirements_type,requirements:mellow_bind_requirements(id,type,data)),oauth_authorisations:mellow_server_oauth_authorisations(expires_at,token_type,access_token,refresh_token)")
				.eq("id", guild_id.to_string())
				.limit(1)
				.single()
				.execute()
				.await?
				.bytes()
				.await?
				.to_vec()
			)?;
			tracing::debug!("servers.write (guild_id={guild_id})");
			
			self.servers.insert(guild_id, new_item.into());
			self.servers.get(&guild_id).unwrap()
		})
	}

	pub async fn event_document(&self, guild_id: Id<GuildMarker>, document_kind: DocumentKind) -> Result<Ref<'_, Uuid, Document>> {
		let key = (guild_id, document_kind.clone());
		Ok(if let Some(item) = self.event_documents.get(&key) {
			tracing::debug!("event_documents.read (guild_id={guild_id}) (document_kind={document_kind:?})");
			HAKUMI_MODELS.vs_document(item.value()).await?
		} else {
			let new_item: Document = simd_json::from_slice(&mut DATABASE.from("visual_scripting_documents")
				.select("id,name,kind,active,definition")
				.eq("kind", document_kind.to_string())
				.eq("mellow_server_id", guild_id.to_string())
				.limit(1)
				.single()
				.execute()
				.await?
				.bytes()
				.await?
				.to_vec()
			)?;
			tracing::debug!("event_documents.write (guild_id={guild_id}) (document_kind={document_kind:?})");
			
			let id = new_item.id.clone();
			self.event_documents.insert((guild_id, document_kind), id.clone());
			HAKUMI_MODELS.vs_documents.insert(id.clone(), new_item.into());
			HAKUMI_MODELS.vs_documents.get(&id).unwrap()
		})
	}
}