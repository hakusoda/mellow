use uuid::Uuid;
use serde::{ de::Deserializer, Deserialize };

use super::connection::{ Connection, ConnectionKind };
use crate::model::mellow::server::UserSettings;

#[derive(Clone, Debug, Deserialize)]
pub struct User {
	pub id: Uuid,
	pub connections: Vec<Connection>,
	#[serde(deserialize_with = "deserialise_user_server_settings")]
	pub server_settings: [UserSettings; 1]
}

impl User {
	pub fn server_settings(&self) -> &UserSettings {
		&self.server_settings[0]
	}

	pub fn server_connections(&self) -> Vec<&Connection> {
		let server_connections = &self.server_settings().user_connections;
		self.connections.iter().filter(|x| server_connections.iter().find(|y| y.id == x.id).is_some()).collect()
	}

	pub fn has_connection(&self, sub: &str, connection_kind: ConnectionKind) -> bool {
		self.connections.iter().any(|x| x.kind == connection_kind && x.sub == sub)
	}
}

fn deserialise_user_server_settings<'de, D: Deserializer<'de>>(deserialiser: D) -> core::result::Result<[UserSettings; 1], D::Error> {
	Vec::<UserSettings>::deserialize(deserialiser)
		.map(|x| match x.is_empty() {
			true => [UserSettings::default()],
			false => [x[0].clone()]
		})
}