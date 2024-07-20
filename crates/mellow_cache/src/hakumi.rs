use dashmap::{
	mapref::{ one::Ref, multiple::RefMulti },
	DashMap,
	DashSet
};
use mellow_models::hakumi::{ user::ConnectionModel, DocumentModel, UserModel };
use mellow_util::hakuid::{
	marker::{ ConnectionMarker, DocumentMarker, UserMarker as HakuUserMarker },
	HakuId
};
use twilight_model::id::{
	marker::{ GuildMarker, UserMarker },
	Id
};

use crate::Result;

#[derive(Default)]
pub struct HakumiCache {
	pub connections: DashMap<HakuId<ConnectionMarker>, ConnectionModel>,
	users: DashMap<HakuId<HakuUserMarker>, UserModel>,
	pub user_connections: DashMap<HakuId<HakuUserMarker>, DashSet<HakuId<ConnectionMarker>>>,
	#[allow(clippy::type_complexity)]
	users_by_discord: DashMap<(Id<GuildMarker>, Id<UserMarker>), Option<HakuId<HakuUserMarker>>>,
	pub visual_scripting_documents: DashMap<HakuId<DocumentMarker>, DocumentModel>
}

impl HakumiCache {
	pub async fn connection(&self, connection_id: HakuId<ConnectionMarker>) -> Result<Ref<'_, HakuId<ConnectionMarker>, ConnectionModel>> {
		Ok(match self.connections.get(&connection_id) {
			Some(model) => model,
			None => {
				let new_model = ConnectionModel::get(connection_id)
					.await?
					.unwrap();
				self.connections.entry(connection_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn connections(&self, connection_ids: &[HakuId<ConnectionMarker>]) -> Result<Vec<RefMulti<'_, HakuId<ConnectionMarker>, ConnectionModel>>> {
		let missing_ids: Vec<_> = connection_ids
			.iter()
			.filter(|x| !self.connections.contains_key(x))
			.copied()
			.collect();
		if !missing_ids.is_empty() {
			let models = ConnectionModel::get_many(&missing_ids)
				.await?;
			for model in models {
				self.connections.insert(model.id, model);
			}
		}
	
		Ok(self.connections
			.iter()
			.filter(|x| connection_ids.contains(&x.id))
			.collect()
		)
	}

	pub async fn user(&self, user_id: HakuId<HakuUserMarker>) -> Result<Ref<'_, HakuId<HakuUserMarker>, UserModel>> {
		Ok(match self.users.get(&user_id) {
			Some(model) => model,
			None => {
				let new_model = UserModel::get(user_id)
					.await?
					.unwrap();
				self.users.entry(user_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn user_connections(&self, user_ids: &[HakuId<HakuUserMarker>]) -> Result<Vec<HakuId<ConnectionMarker>>> {
		let missing_ids: Vec<_> = user_ids
			.iter()
			.filter(|x| !self.user_connections.contains_key(x))
			.copied()
			.collect();
		if !missing_ids.is_empty() {
			let connection_ids = UserModel::connections_many(&missing_ids)
				.await?;
			for (user_id, connection_ids) in connection_ids.into_iter() {
				self.user_connections
					.entry(user_id)
					.or_default()
					.extend(connection_ids.into_iter());
			}
		}
	
		Ok(self.user_connections
			.iter()
			.filter(|x| user_ids.contains(x.key()))
			.flat_map(|x| x.iter().map(|x| *x).collect::<Vec<_>>())
			.collect()
		)
	}

	pub async fn user_by_discord(&self, guild_id: Id<GuildMarker>, user_id: Id<UserMarker>) -> Result<Option<HakuId<HakuUserMarker>>> {
		let key = (guild_id, user_id);
		Ok(if let Some(item) = self.users_by_discord.get(&key) {
			*item
		} else {
			let user_id = ConnectionModel::user_discord(user_id)
				.await?;
			self.users_by_discord.insert(key, user_id);
			user_id
		})
	}

	pub async fn visual_scripting_document(&self, document_id: HakuId<DocumentMarker>) -> Result<Ref<'_, HakuId<DocumentMarker>, DocumentModel>> {
		Ok(match self.visual_scripting_documents.get(&document_id) {
			Some(model) => model,
			None => {
				let new_model = DocumentModel::get(document_id)
					.await?
					.unwrap();
				self.visual_scripting_documents.entry(document_id)
					.insert(new_model)
					.downgrade()
			}
		})
	}

	pub async fn visual_scripting_documents(&self, document_ids: &[HakuId<DocumentMarker>]) -> Result<Vec<RefMulti<'_, HakuId<DocumentMarker>, DocumentModel>>> {
		let missing_ids: Vec<_> = document_ids
			.iter()
			.filter(|x| !self.visual_scripting_documents.contains_key(x))
			.copied()
			.collect();
		if !missing_ids.is_empty() {
			let models = DocumentModel::get_many(&missing_ids)
				.await?;
			for model in models {
				self.visual_scripting_documents.insert(model.id, model);
			}
		}
	
		Ok(self.visual_scripting_documents
			.iter()
			.filter(|x| document_ids.contains(&x.id))
			.collect()
		)
	}
}