use serde::Deserialize;

use crate::model::hakumi::{
	id::{
		marker::ConnectionMarker,
		HakuId
	},
	user::{ User, Connection }
};

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UserSettings {
	pub user_connections: Vec<ConnectionReference>
}

impl UserSettings {
	pub fn server_connections<'a>(&self, user: &'a User) -> Vec<&'a Connection> {
		let references = &self.user_connections;
		user.connections.iter().filter(|x| references.iter().any(|y| y.id == x.id)).collect()
	}
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionReference {
	pub id: HakuId<ConnectionMarker>
}