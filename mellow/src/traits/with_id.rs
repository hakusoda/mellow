pub trait WithId<T> {
	fn with_id<I>(self, id: I) -> crate::util::WithId<I, T>;
}

impl<T> WithId<T> for T {
	fn with_id<I>(self, id: I) -> crate::util::WithId<I, T> {
		crate::util::WithId {
			id,
			inner: self
		}
	}
}