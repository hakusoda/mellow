#![feature(const_async_blocks, type_alias_impl_trait)]
use async_once_cell::Lazy as AsyncLazy;
use chrono::Utc;
use once_cell::sync::Lazy;
use rand::{ distributions::Alphanumeric, Rng };
use sqlx::PgPool;
use std::pin::Pin;
use twilight_http::{ client::InteractionClient, Client };
use twilight_model::id::{ marker::ApplicationMarker, Id };

pub mod fetch;
pub use fetch::*;

pub mod hakuid;
pub use hakuid::HakuId;
use hakuid::marker::UserMarker;

pub static DISCORD_CLIENT: Lazy<Client> = Lazy::new(|| Client::new(env!("DISCORD_BOT_TOKEN").to_owned()));
pub static DISCORD_INTERACTION_CLIENT: Lazy<InteractionClient> = Lazy::new(||
	DISCORD_CLIENT.interaction(*DISCORD_APP_ID)
);

pub static DISCORD_APP_ID: Lazy<Id<ApplicationMarker>> = Lazy::new(|| env!("DISCORD_APP_ID").to_owned().parse().unwrap());

pub type PgPoolFuture = impl Future<Output = PgPool>;
pub static PG_POOL: AsyncLazy<PgPool, PgPoolFuture> = AsyncLazy::new(async {
	PgPool::connect(env!("DATABASE_URL"))
		.await
		.unwrap()
});

pub async fn create_website_token(user_id: HakuId<UserMarker>) -> Result<String, sqlx::Error> {
	let value: String = rand::thread_rng()
		.sample_iter(Alphanumeric)
		.take(24)
		.map(char::from)
		.collect();
	sqlx::query!(
		"
		INSERT INTO mellow_website_tokens (user_id, value)
		VALUES ($1, $2)
		ON CONFLICT (user_id)
		DO UPDATE SET value = $2, created_at = $3
		",
		user_id.value,
		&value,
		Utc::now()
	)
		.execute(&*Pin::static_ref(&PG_POOL).await)
		.await?;

	Ok(value)
}