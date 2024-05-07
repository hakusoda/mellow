use std::fmt::Debug;

use serde::de::DeserializeOwned;
use reqwest::{
	header::HeaderMap,
	Client, Method, IntoUrl
};
use once_cell::sync::Lazy;
use crate::Result;

pub static CLIENT: Lazy<Client> = Lazy::new(||
	Client::builder()
		.default_headers({
			let mut headers = HeaderMap::new();
			headers.append("accept", "application/json".parse().unwrap());
			headers.append("authorization", format!("Bot {}", env!("DISCORD_TOKEN")).parse().unwrap());
			headers
		})
		.build()
		.unwrap()
);

#[tracing::instrument(skip(body, headers))]
pub async fn fetch_json<U: IntoUrl + Debug, T: DeserializeOwned>(url: U, method: Option<Method>, body: Option<serde_json::Value>, headers: Option<HeaderMap>) -> Result<T> {
	let url = url.into_url()?;
	let mut builder = CLIENT.request(method.unwrap_or(Method::GET), url.clone());
	if let Some(body) = body {
		builder = builder.json(&body);
	}
	if let Some(headers) = headers {
		builder = builder.headers(headers);
	}

	match builder.send().await {
		Ok(x) => {
			Ok(if std::any::type_name::<T>() == std::any::type_name::<()>() {
				serde_json::from_value(serde_json::Value::Null)?
			} else {
				simd_json::from_slice(&mut x.bytes().await?.to_vec())?
			})
		},
		Err(error) => Err(crate::error::ErrorKind::FormattedHttpError(url.to_string(), error.to_string()).into())
	}
}

pub async fn get_json<U: IntoUrl + Debug, T: DeserializeOwned>(url: U, headers: Option<HeaderMap>) -> Result<T> {
	fetch_json(url, Some(Method::GET), None, headers).await
}