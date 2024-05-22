use std::collections::HashMap;
use serde::{ Serialize, Deserialize };
use twilight_model::id::{
	marker::GuildMarker,
	Id
};

use crate::{
	model::discord::{
		guild::CachedMember,
		DISCORD_MODELS
	},
	Result
};

#[derive(Eq, Clone, Debug, PartialEq)]
pub struct Variable {
	pub kind: VariableKind,
	pub interpret_as: VariableInterpretAs
}

impl Variable {
	pub fn create_map<const N: usize>(value: [(&str, Self); N], interpret_as: Option<VariableInterpretAs>) -> Self {
		Self {
			kind: VariableKind::Map(value.into_iter().map(|x| (x.0.to_string(), x.1)).collect()),
			interpret_as: interpret_as.unwrap_or(VariableInterpretAs::NonSpecific)
		}
	}
	
	pub fn from_partial_member(user: Option<&twilight_model::user::User>, member: &twilight_model::guild::PartialMember, guild_id: &Id<GuildMarker>) -> Variable {
		let user = user.unwrap_or_else(|| member.user.as_ref().unwrap());
		Variable::create_map([
			("id", user.id.to_string().into()),
			("roles", VariableKind::List(member.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", guild_id.to_string().into()),
			("username", user.name.clone().into()),
			("avatar_url", member.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", user.id)).unwrap_or("".into()).into()),
			("display_name", user.global_name.clone().unwrap_or_else(|| user.name.clone()).into())
		], Some(VariableInterpretAs::Member))
	}

	pub async fn from_member(member: &CachedMember, guild_id: Id<GuildMarker>) -> Result<Variable> {
		let user = DISCORD_MODELS.user(member.user_id).await?;
		Ok(Variable::create_map([
			("id", member.user_id.to_string().into()),
			("roles", VariableKind::List(member.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", guild_id.to_string().into()),
			("username", user.name.clone().into()),
			("avatar_url", member.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", user.id)).unwrap_or("".into()).into()),
			("display_name", user.global_name.clone().unwrap_or_else(|| user.name.clone()).into())
		], Some(VariableInterpretAs::Member)))
	}

	pub fn get(&self, key: &str) -> &Variable {
		self.as_map().unwrap().get(key).unwrap()
	}

	pub fn set(&mut self, key: &str, value: Variable) {
		self.as_map_mut().unwrap().insert(key.into(), value);
	}

	pub fn as_map(&self) -> Option<&HashMap<String, Variable>> {
		match &self.kind {
			VariableKind::Map(x) => Some(x),
			_ => None
		}
	}

	pub fn as_map_mut(&mut self) -> Option<&mut HashMap<String, Variable>> {
		match &mut self.kind {
			VariableKind::Map(x) => Some(x),
			_ => None
		}
	}

	pub fn cast_id<T>(&self) -> Id<T> {
		Id::new(self.cast_str().parse().unwrap())
	}

	pub fn cast_str(&self) -> &str {
		match &self.kind {
			VariableKind::String(x) => x,
			_ => panic!()
		}
	}

	pub fn cast_string(&self) -> String {
		match &self.kind {
			VariableKind::String(x) => x.clone(),
			VariableKind::UnsignedInteger(x) => x.to_string(),
			_ => panic!()
		}
	}

	pub fn is_empty(&self) -> bool {
		match &self.kind {
			VariableKind::Map(x) => x.is_empty(),
			VariableKind::List(x) => x.is_empty(),
			VariableKind::String(x) => x.is_empty(),
			VariableKind::UnsignedInteger(_) => false
		}
	}

	pub fn contains(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::Map(x) => x.iter().any(|x| x.1 == variable),
			VariableKind::List(x) => x.iter().any(|x| x == variable),
			VariableKind::String(x) => x.contains(variable.cast_str()),
			VariableKind::UnsignedInteger(_) => false
		}
	}

	pub fn contains_only(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::List(x) => match &variable.kind {
				VariableKind::List(y) => x.iter().all(|x| y.iter().any(|y| x == y)),
				_ => false
			},
			_ => false
		}
	}

	pub fn contains_one_of(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::List(x) => match &variable.kind {
				VariableKind::List(y) => x.iter().any(|x| y.iter().any(|y| x == y)),
				_ => false
			},
			_ => false
		}
	}

	pub fn starts_with(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::List(x) => x.first().is_some_and(|x| x == variable),
			VariableKind::String(x) => x.starts_with(variable.cast_str()),
			_ => false
		}
	}

	pub fn ends_with(&self, variable: &Variable) -> bool {
		match &self.kind {
			VariableKind::List(x) => x.last().is_some_and(|x| x == variable),
			VariableKind::String(x) => x.ends_with(variable.cast_str()),
			_ => false
		}
	}
}

impl From<String> for Variable {
	fn from(value: String) -> Self {
		Variable {
			kind: VariableKind::String(value),
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

impl From<u64> for Variable {
	fn from(value: u64) -> Self {
		Variable {
			kind: VariableKind::UnsignedInteger(value),
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

impl<T> From<Id<T>> for Variable {
	fn from(value: Id<T>) -> Self {
		value.to_string().into()
	}
}

impl<T: Into<Variable>> From<Vec<T>> for Variable {
	fn from(value: Vec<T>) -> Self {
		Variable {
			kind: VariableKind::List(value.into_iter().map(|x| x.into()).collect()),
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

impl From<twilight_model::user::User> for Variable {
	fn from(value: twilight_model::user::User) -> Self {
		Variable::create_map([
			("id", value.id.to_string().into()),
			("username", value.name.clone().into()),
			("avatar_url", value.avatar.map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", value.id)).unwrap_or("".into()).into()),
			("display_name", value.global_name.unwrap_or(value.name).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl From<&twilight_model::gateway::payload::incoming::MessageCreate> for Variable {
	fn from(value: &twilight_model::gateway::payload::incoming::MessageCreate) -> Self {
		Variable::create_map([
			("id", value.id.to_string().into()),
			("author", value.author.clone().into()),
			("content", value.content.clone().into()),
			("channel_id", value.channel_id.to_string().into())
		], Some(VariableInterpretAs::Message))
	}
}

impl From<&twilight_model::gateway::payload::incoming::MemberUpdate> for Variable {
	fn from(value: &twilight_model::gateway::payload::incoming::MemberUpdate) -> Self {
		Variable::create_map([
			("id", value.user.id.to_string().into()),
			("roles", VariableKind::List(value.roles.iter().map(|x| x.to_string().into()).collect()).into()),
			("guild_id", value.guild_id.to_string().into()),
			("username", value.user.name.clone().into()),
			("avatar_url", value.avatar.or(value.user.avatar).map(|x| format!("https://cdn.discordapp.com/avatars/{}/{x}.webp", value.user.id)).unwrap_or("".into()).into()),
			("display_name", value.user.global_name.clone().unwrap_or_else(|| value.user.name.clone()).into())
		], Some(VariableInterpretAs::Member))
	}
}

impl From<&serde_json::Value> for Variable {
	fn from(value: &serde_json::Value) -> Self {
		use serde_json::Value;
		match value {
			Value::Null => unimplemented!(),
			Value::Bool(_) => unimplemented!(),
			Value::Array(x) => VariableKind::List(x.iter().map(|x| x.into()).collect()),
			Value::Number(_) => unimplemented!(),
			Value::Object(x) => VariableKind::Map(x.iter().map(|x| (x.0.clone(), x.1.into())).collect()),
			Value::String(x) => VariableKind::String(x.clone())
		}.into()
	}
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum VariableKind {
	Map(HashMap<String, Variable>),
	List(Vec<Variable>),

	String(String),
	UnsignedInteger(u64)
}

impl From<VariableKind> for Variable {
	fn from(value: VariableKind) -> Self {
		Variable {
			kind: value,
			interpret_as: VariableInterpretAs::NonSpecific
		}
	}
}

#[derive(Eq, Clone, Debug, PartialEq)]
pub enum VariableInterpretAs {
	NonSpecific,
	Member,
	Message
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct VariableReference {
	path: String
}

impl VariableReference {
	pub fn resolve(&self, root_variable: &Variable) -> Option<Variable> {
		let mut variable: Option<&Variable> = None;
		for key in self.path.split("::") {
			variable = Some(match &match variable {
				Some(x) => x,
				_ => root_variable
			}.kind {
				VariableKind::Map(map) => match map.get(key) {
					Some(x) => x,
					_ => return None
				},
				VariableKind::List(list) => match list.get(key.parse::<usize>().unwrap()) {
					Some(x) => x,
					_ => return None
				},
				_ => return None
			});
		}

		variable.cloned()
	}
}