use std::fmt::Debug;

use serde::{ de::DeserializeOwned, Serialize };
use reqwest::{
	header::HeaderMap,
	Client, Method, IntoUrl
};
use once_cell::sync::Lazy;
use crate::Result;

pub const CLIENT: Lazy<Client> = Lazy::new(||
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

#[tracing::instrument]
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
				//x.json().await.map_err(|x| crate::error::ErrorKind::FormattedHttpError(url.to_string(), x.to_string()))?
				let text = x.text().await?;
				println!("{text}");
				
				serde_json::from_str(&text)?
			})
		},
		Err(error) => Err(crate::error::ErrorKind::FormattedHttpError(url.to_string(), error.to_string()).into())
	}
}

pub async fn get_json<U: IntoUrl + Debug, T: DeserializeOwned>(url: U, headers: Option<HeaderMap>) -> Result<T> {
	fetch_json(url, Some(Method::GET), None, headers).await
}

pub async fn post_json<U: IntoUrl + Debug, T: DeserializeOwned, B: Serialize>(url: U, body: B) -> Result<T> {
	fetch_json(url, Some(Method::POST), Some(serde_json::to_value(body)?), None).await
}

pub async fn patch_json<U: IntoUrl + Debug, T: DeserializeOwned, B: Serialize>(url: U, body: B) -> Result<T> {
	fetch_json(url, Some(Method::PATCH), Some(serde_json::to_value(body)?), None).await
}