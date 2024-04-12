use uuid::Uuid;
use serde::Deserialize;

#[derive(Clone, Debug, Default, Deserialize)]
pub struct UserSettings {
	pub user_connections: Vec<ConnectionReference>
}

#[derive(Clone, Debug, Deserialize)]
pub struct ConnectionReference {
	pub id: Uuid
}