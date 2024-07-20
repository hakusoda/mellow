use futures::TryStreamExt;
use mellow_util::{
	hakuid::{
		marker::DocumentMarker,
		HakuId
	},
	PG_POOL
};
use std::pin::Pin;
use twilight_model::id::{
	marker::{ CommandMarker, GuildMarker },
	Id
};

use crate::Result;

#[derive(Clone, Debug)]
pub struct CommandModel {
	pub id: Id<CommandMarker>,
	pub document_id: HakuId<DocumentMarker>,
	pub is_ephemeral: bool
}

impl CommandModel {
	pub async fn get(command_id: Id<CommandMarker>) -> Result<Option<Self>> {
		Self::get_many(&[command_id])
			.await
			.map(|x| x.into_iter().next())
	}

	pub async fn get_many(command_ids: &[Id<CommandMarker>]) -> Result<Vec<Self>> {
		if command_ids.is_empty() {
			return Ok(vec![]);
		}

		let command_ids: Vec<i64> = command_ids
			.iter()
			.map(|x| x.get() as i64)
			.collect();
		Ok(sqlx::query!(
			r#"
			SELECT id, document_id, is_ephemeral
			FROM mellow_server_commands
			WHERE id = ANY($1)
			"#,
			&command_ids
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, u| {
				acc.push(Self {
					id: Id::new(u.id as u64),
					document_id: u.document_id.into(),
					is_ephemeral: u.is_ephemeral
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}

	pub async fn get_server_many(guild_id: Id<GuildMarker>) -> Result<Vec<Self>> {
		Ok(sqlx::query!(
			r#"
			SELECT id, document_id, is_ephemeral
			FROM mellow_server_commands
			WHERE server_id = $1
			"#,
			guild_id.get() as i64
		)
			.fetch(&*Pin::static_ref(&PG_POOL).await)
			.try_fold(Vec::new(), |mut acc, u| {
				acc.push(Self {
					id: Id::new(u.id as u64),
					document_id: u.document_id.into(),
					is_ephemeral: u.is_ephemeral
				});

				async move { Ok(acc) }
			})
			.await?
		)
	}
}