use once_cell::sync::Lazy;
use reqwest::{
	header::{ HeaderName, HeaderValue },
	Client, Error, IntoUrl, Method, RequestBuilder
};
use serde::{ de::DeserializeOwned, Serialize };
use std::{
	future::IntoFuture,
	marker::PhantomData
};

pub static HTTP: Lazy<Client> = Lazy::new(Client::new);

pub struct FetchJson<T: DeserializeOwned> {
	phantom: PhantomData<T>,
	request: RequestBuilder
}

impl<T: DeserializeOwned> FetchJson<T> {
	pub fn new(request: RequestBuilder) -> Self {
		Self {
			phantom: PhantomData,
			request
		}
	}

	pub fn header<K, V>(mut self, key: K, value: V) -> Self
	where
		HeaderName: TryFrom<K>,
		<HeaderName as TryFrom<K>>::Error: Into<http::Error>,
		HeaderValue: TryFrom<V>,
		<HeaderValue as TryFrom<V>>::Error: Into<http::Error>,
	{
		self.request = self.request.header(key, value);
		self
	}

	pub fn form<F: Serialize + ?Sized>(mut self, form: &F) -> Self {
		self.request = self.request.form(form);
		self
	}
}

pub type FetchJsonFuture<T: DeserializeOwned> = impl Future<Output = Result<T, Error>>;

impl<T: DeserializeOwned> IntoFuture for FetchJson<T> {
	type IntoFuture = FetchJsonFuture<T>;
	type Output = Result<T, Error>;

	fn into_future(self) -> Self::IntoFuture {
		async move {
			self
				.request
				.send()
				.await?
				.json()
				.await
		}
	}
}

pub fn fetch_json<T: DeserializeOwned, U: IntoUrl>(url: U, method: Method) -> FetchJson<T> {
	FetchJson::new(HTTP.request(method, url))
}

pub fn get_json<T: DeserializeOwned, U: IntoUrl>(url: U) -> FetchJson<T> {
	FetchJson::new(HTTP.get(url))
}

pub fn post_json<T: DeserializeOwned, U: IntoUrl>(url: U) -> FetchJson<T> {
	FetchJson::new(HTTP.post(url))
}