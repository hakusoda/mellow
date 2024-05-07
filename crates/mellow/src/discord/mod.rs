use once_cell::sync::Lazy;
use twilight_http::{ client::InteractionClient, Client };
use twilight_model::id::{
	marker::ApplicationMarker,
	Id
};

pub mod gateway;

pub static APP_ID: Lazy<Id<ApplicationMarker>> = Lazy::new(|| Id::new(env!("DISCORD_APP_ID").parse().unwrap()));
pub static CLIENT: Lazy<Client> = Lazy::new(|| Client::new(env!("DISCORD_TOKEN").into()));
pub static INTERACTION: Lazy<InteractionClient> = Lazy::new(|| CLIENT.interaction(*APP_ID));