use serde::{ Deserialize, Deserializer };

pub fn deserialise_nullable_vec<'de, D: Deserializer<'de>, T: Deserialize<'de>>(deserialiser: D) -> Result<Vec<T>, D::Error> {
	Ok(Vec::deserialize(deserialiser).unwrap_or(vec![]))
}