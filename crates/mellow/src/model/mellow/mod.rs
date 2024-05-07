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
	model::hakumi::{
		id::{
			marker::{ UserMarker, DocumentMarker },
			HakuId
		},
		HAKUMI_MODELS
	},
	database::DATABASE,
	visual_scripting::{ Document, DocumentKind },
	Result
};
use server::{ Server, UserSettings };

pub mod server;

pub static MELLOW_MODELS: Lazy<MellowModels> = Lazy::new(MellowModels::default);

#[derive(Debug, Default)]
pub struct MellowModels {
	pub servers: DashMap<Id<GuildMarker>, Server>,
	pub event_documents: DashMap<(Id<GuildMarker>, DocumentKind), Option<HakuId<DocumentMarker>>>,
	pub member_settings: DashMap<(Id<GuildMarker>, HakuId<UserMarker>), UserSettings>,
}

impl MellowModels {
	pub async fn server(&self, guild_id: Id<GuildMarker>) -> Result<Ref<'_, Id<GuildMarker>, Server>> {
		Ok(if let Some(item) = self.servers.get(&guild_id) {
			tracing::debug!("servers.read (guild_id={guild_id})");
			item
		} else {
			let new_item: Server = DATABASE.from("mellow_servers")
				.select("id, actions:mellow_server_sync_actions( id, kind, criteria, action_data, display_name ), logging_types, default_nickname, logging_channel_id, allow_forced_syncing, oauth_authorisations:mellow_server_oauth_authorisations( id, expires_at, token_type, access_token, refresh_token )")
				.eq("id", guild_id.to_string())
				.limit(1)
				.single()
				.await?
				.value;
			tracing::debug!("servers.write (guild_id={guild_id})");
			
			self.servers.insert(guild_id, new_item);
			self.servers.get(&guild_id).unwrap()
		})
	}

	pub async fn event_document(&self, guild_id: Id<GuildMarker>, document_kind: DocumentKind) -> Result<Option<Ref<'_, HakuId<DocumentMarker>, Document>>> {
		let key = (guild_id, document_kind.clone());
		Ok(if let Some(item) = self.event_documents.get(&key) {
			tracing::debug!("event_documents.read (guild_id={guild_id}) (document_kind={document_kind:?})");
			if let Some(id) = *item {
				Some(HAKUMI_MODELS.vs_document(id).await?)
			} else { None }
		} else {
			let new_item: Option<Document> = DATABASE.from("visual_scripting_documents")
				.select("id,name,kind,active,definition")
				.eq("kind", document_kind.to_string())
				.eq("mellow_server_id", guild_id.to_string())
				.limit(1)
				.maybe_single()
				.await?
				.value;
			tracing::debug!("event_documents.write (guild_id={guild_id}) (document_kind={document_kind:?})");
			
			let id: Option<HakuId<DocumentMarker>> = new_item.as_ref().map(|x| x.id);
			self.event_documents.insert((guild_id, document_kind), id);
			
			if let Some(new_item) = new_item && let Some(id) = id {
				HAKUMI_MODELS.vs_documents.insert(id, new_item);
				HAKUMI_MODELS.vs_documents.get(&id)
			} else { None }
		})
	}

	pub async fn member_settings(&self, guild_id: Id<GuildMarker>, user_id: HakuId<UserMarker>) -> Result<Ref<'_, (Id<GuildMarker>, HakuId<UserMarker>), UserSettings>> {
		let key = (guild_id, user_id);
		Ok(if let Some(item) = self.member_settings.get(&key) {
			tracing::debug!("member_settings.read (guild_id={guild_id}) (user_id={user_id:?})");
			item
		} else {
			let new_item: UserSettings = DATABASE.from("mellow_user_server_settings")
				.select("user_connections")
				.eq("user_id", user_id.to_string())
				.eq("server_id", guild_id.to_string())
				.limit(1)
				.single()
				.await?
				.value;
			tracing::debug!("member_settings.write (guild_id={guild_id}) (user_id={user_id})");
			
			self.member_settings.insert(key, new_item.clone());
			self.member_settings.get(&key).unwrap()
		})
	}
}