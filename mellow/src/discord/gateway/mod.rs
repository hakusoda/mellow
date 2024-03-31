use twilight_model::{
	gateway::{
		payload::{incoming::MemberChunk, outgoing::{
			update_presence::UpdatePresencePayload,
			RequestGuildMembers
		}},
		presence::{ Activity, ActivityType, Status }
	}, guild::Member, id::Id
};
use twilight_gateway::{ Event, Shard, Config, Intents, ShardId };

use crate::server::{ logging::ServerLog, Server };

pub mod event_handler;

#[tracing::instrument]
pub async fn initialise() {
	let config = Config::builder(env!("DISCORD_TOKEN").to_string(), Intents::GUILD_MEMBERS | Intents::MESSAGE_CONTENT | Intents::GUILD_MESSAGES)
		.presence(UpdatePresencePayload::new(vec![Activity {
			id: None,
			url: None,
			name: "burgers".into(),
			kind: ActivityType::Custom,
			emoji: None,
			flags: None,
			party: None,
			state: Some("now here's the syncer".into()),
			assets: None,
			buttons: vec![],
			details: None,
			secrets: None,
			instance: None,
			created_at: None,
			timestamps: None,
			application_id: None
		}.into()], false, None, Status::Online).unwrap())
		.build();
	let mut shard = Shard::with_config(ShardId::ONE, config);
	while !shard.status().is_identified() {
		shard.next_message().await.unwrap();
	}

	/*let a = RequestGuildMembers::builder(Id::new(346444423271415819))
		.query("", Some(9660));

	let mut members: Vec<Member> = vec![];
	shard.command(&a).await.unwrap();*/
	loop {
		let event = match shard.next_event().await {
			Ok(event) => event,
			Err(source) => {
				tracing::warn!(?source, "error receiving event");
				if source.is_fatal() {
					break;
				}

				continue;
			}
		};

		match event {
			Event::MemberAdd(event_data) => {
				tokio::spawn(async move {
					if let Err(error) = event_handler::member_add(&event_data).await {
						Server::fetch(event_data.guild_id.to_string()).await.unwrap().send_logs(vec![ServerLog::VisualScriptingProcessorError {
							error: error.to_string(),
							document_name: "New Member Event".into()
						}]).await.unwrap();
					}
				});
			},
			Event::MemberUpdate(event_data) => {
				tokio::spawn(async move {
					event_handler::member_update(&event_data).await.unwrap();
				});
			},
			Event::MessageCreate(event_data) => {
				if !event_data.author.bot {
					tokio::spawn(async move {
						event_handler::message_create(&event_data).await.unwrap();
					});
				}
			},
			/*Event::MemberChunk(event_data) => {
				{
					for member in event_data.members {
						members.push(member.clone());
					}
				}
				if event_data.chunk_index == event_data.chunk_count - 1 {
					let i = members.clone();
					tokio::spawn(async move {
						for member in i.into_iter() {
							if member.roles.is_empty() && !member.pending {
								println!("giving {} the unverified role", member.user.name);
								crate::discord::assign_member_role(event_data.guild_id.to_string(), member.user.id.to_string(), "1223305945929744424").await.unwrap();
								tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
							}
						}
						println!("done");
					});
				}
			},*/
			_ => ()
		}
	}
}