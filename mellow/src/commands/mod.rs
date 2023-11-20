use once_cell::sync::Lazy;

use crate::Command;

pub mod syncing;

pub static COMMANDS: Lazy<Vec<Command>> = Lazy::new(|| vec![
	syncing::sync(),
	syncing::forcesyncall()
]);