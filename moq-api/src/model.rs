use axum::http;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Default)]
pub struct Broadcasts {
	pub active: HashMap<String, Broadcast>,
}

#[derive(Serialize, Deserialize)]
pub struct Broadcast {
	#[serde(with = "http_serde::uri")]
	pub origin: http::Uri,
}
