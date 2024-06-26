use once_cell::sync::Lazy;

use crate::Command;

pub mod server;
pub mod syncing;

pub static COMMANDS: Lazy<Vec<Command>> = Lazy::new(|| vec![
	server::setup(),
	
	syncing::sync(),
	syncing::forcesync(),
	syncing::forcesyncall()
]);