use dashmap::{
	mapref::{
		one::{ Ref, RefMut },
		multiple::RefMulti
	},
	DashMap, DashSet
};
use mellow_models::{
	hakumi::{ visual_scripting::DocumentKind, DocumentModel, OAuthAuthorisationModel },
	mellow::{
		server::{
			CommandModel,
			ServerModel,
			SyncActionModel,
			UserSettingsModel
		},
		SignUpModel
	}
};
use mellow_util::hakuid::{
	marker::{ DocumentMarker, SyncActionMarker, UserMarker as HakuUserMarker },
	HakuId
};
use twilight_model::id::{
	marker::{ CommandMarker, GuildMarker, UserMarker },
	Id
};

use crate::{ CACHE, Result };

#[derive(Default)]
pub struct MellowCache {
	commands: DashMap<Id<CommandMarker>, CommandModel>,
	pub oauth_authorisations: DashMap<u64, OAuthAuthorisationModel>,
	pub servers: DashMap<Id<GuildMarker>, ServerModel>,
	server_oauth_authorisations: DashMap<Id<GuildMarker>, DashSet<u64>>,
	server_sync_actions: DashMap<Id<GuildMarker>, DashSet<HakuId<SyncActionMarker>>>,
	pub server_visual_scripting_documents: DashMap<Id<GuildMarker>, DashSet<HakuId<DocumentMarker>>>,
	pub sign_ups: DashMap<Id<UserMarker>, SignUpModel>,
	sync_actions: DashMap<HakuId<SyncActionMarker>, SyncActionModel>,
	pub user_settings: DashMap<(Id<GuildMarker>, HakuId<HakuUserMarker>), UserSettingsModel>
}

impl MellowCache {
	pub async fn command(&self, command_id: Id<CommandMarker>) -> Result<Ref<'_, Id<CommandMarker>, CommandModel>> {
		Ok(match self.commands.get(&command_id) {
			Some(model) => model,
			None => {
				let new_model = CommandModel::get(command_id)
					.await?
					.unwrap();
				self.commands
					.entry(command_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn event_document(&self, guild_id: Id<GuildMarker>, document_kind: DocumentKind) -> Result<Option<RefMulti<'_, HakuId<DocumentMarker>, DocumentModel>>> {
		let document_ids = self.server_visual_scripting_documents(guild_id)
			.await?;
		Ok(CACHE
			.hakumi
			.visual_scripting_documents(&document_ids)
			.await?
			.into_iter()
			.find(|x| x.kind == document_kind)
		)
	}

	pub fn oauth_authorisation(&self, oauth_authorisation_id: u64) -> Option<Ref<'_, u64, OAuthAuthorisationModel>> {
		self.oauth_authorisations.get(&oauth_authorisation_id)
	}

	pub fn server(&self, guild_id: Id<GuildMarker>) -> Option<Ref<'_, Id<GuildMarker>, ServerModel>> {
		self.servers.get(&guild_id)
	}

	pub fn server_mut(&self, guild_id: Id<GuildMarker>) -> Option<RefMut<Id<GuildMarker>, ServerModel>> {
		self.servers.get_mut(&guild_id)
	}
	
	pub async fn server_oauth_authorisations(&self, guild_id: Id<GuildMarker>) -> Result<Vec<u64>> {
		Ok(match self.server_oauth_authorisations.get(&guild_id) {
			Some(model) => model
				.iter()
				.map(|x| *x)
				.collect(),
			None => {
				let models = ServerModel::oauth_authorisations(guild_id)
					.await?;
				let model_ids: Vec<_> = models
					.iter()
					.map(|x| x.id)
					.collect();
				for model in models {
					self.oauth_authorisations.insert(model.id, model);
				}

				self.server_oauth_authorisations
					.entry(guild_id)
					.or_default()
					.extend(model_ids.clone());
				model_ids
			}
		})
	}

	pub async fn server_sync_actions(&self, guild_id: Id<GuildMarker>) -> Result<Vec<HakuId<SyncActionMarker>>> {
		Ok(match self.server_sync_actions.get(&guild_id) {
			Some(model) => model
				.iter()
				.map(|x| *x)
				.collect(),
			None => {
				let model_ids = ServerModel::sync_actions(guild_id)
					.await?;
				self.server_sync_actions
					.entry(guild_id)
					.or_default()
					.extend(model_ids.clone());
				model_ids
			}
		})
	}

	pub async fn server_visual_scripting_documents(&self, guild_id: Id<GuildMarker>) -> Result<Vec<HakuId<DocumentMarker>>> {
		Ok(match self.server_visual_scripting_documents.get(&guild_id) {
			Some(model) => model
				.iter()
				.map(|x| *x)
				.collect(),
			None => {
				let model_ids = ServerModel::visual_scripting_documents(guild_id)
					.await?;
				self.server_visual_scripting_documents
					.entry(guild_id)
					.or_default()
					.extend(model_ids.clone());
				model_ids
			}
		})
	}

	pub async fn sync_actions(&self, sync_action_ids: &[HakuId<SyncActionMarker>]) -> Result<Vec<RefMulti<'_, HakuId<SyncActionMarker>, SyncActionModel>>> {
		let missing_ids: Vec<_> = sync_action_ids
			.iter()
			.filter(|x| !self.sync_actions.contains_key(x))
			.copied()
			.collect();
		if !missing_ids.is_empty() {
			let models = SyncActionModel::get_many(&missing_ids)
				.await?;
			for model in models {
				self.sync_actions.insert(model.id, model);
			}
		}
	
		Ok(self.sync_actions
			.iter()
			.filter(|x| sync_action_ids.contains(&x.id))
			.collect()
		)
	}

	pub async fn user_settings(&self, guild_id: Id<GuildMarker>, user_id: HakuId<HakuUserMarker>) -> Result<Ref<'_, (Id<GuildMarker>, HakuId<HakuUserMarker>), UserSettingsModel>> {
		let key = (guild_id, user_id);
		Ok(match self.user_settings.get(&key) {
			Some(model) => model,
			None => {
				let new_model = UserSettingsModel::get(guild_id, user_id)
					.await?;
				self.user_settings
					.entry(key)
					.insert(new_model)
					.downgrade()
			}
		})
	}
}