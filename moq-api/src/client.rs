use axum::http;

use crate::Broadcast;

#[derive(Clone)]
pub struct Client {
	uri: http::Uri,
}

impl Client {
	pub fn new(uri: http::Uri) -> Self {
		Self { uri }
	}

	pub async fn get_broadcast(&self, _id: &str) -> anyhow::Result<Option<Broadcast>> {
		unimplemented!("get broadcast from API");
	}

	pub async fn set_broadcast(&mut self, _id: &str, _broadcast: Broadcast) -> anyhow::Result<()> {
		unimplemented!("set broadcast from API");
	}

	pub async fn delete_broadcast(&mut self, _id: &str) -> anyhow::Result<()> {
		unimplemented!("delete broadcast from API");
	}
}
