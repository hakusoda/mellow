use std::{
	cmp::Ordering,
	fmt::{ Debug, Display },
	hash::{ Hash, Hasher },
	marker::PhantomData
};
use uuid::Uuid;
use serde::{ Serialize, Serializer, Deserialize, Deserializer };

pub mod marker;

pub struct HakuId<T> {
	pub value: Uuid,
	phantom: PhantomData<fn(T) -> T>
}

impl<T> HakuId<T> {
	pub const fn new(value: Uuid) -> Self {
		Self {
			value,
			phantom: PhantomData
		}
	}
}

impl<T> Clone for HakuId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for HakuId<T> {}

impl<T> Debug for HakuId<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Debug::fmt(&self.value, f)
	}
}

impl<T> Eq for HakuId<T> {}

impl<T> Hash for HakuId<T> {
    fn hash<U: Hasher>(&self, state: &mut U) {
        self.value.hash(state)
    }
}

impl<T> Ord for HakuId<T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<T> PartialOrd for HakuId<T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<T> PartialEq for HakuId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.value == other.value
    }
}

impl<T> Display for HakuId<T> {
	fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
		Display::fmt(&self.value, f)
	}
}

impl<T> From<Uuid> for HakuId<T> {
	fn from(value: Uuid) -> Self {
		Self::new(value)
	}
}

impl<T> Serialize for HakuId<T> {
	fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
		self.value.serialize(serializer)
	}
}

impl<'de, T> Deserialize<'de> for HakuId<T> {
	fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
		Ok(Self::new(Uuid::deserialize(deserializer)?))
	}
}