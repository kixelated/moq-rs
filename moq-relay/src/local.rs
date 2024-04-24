use std::collections::hash_map;
use std::collections::HashMap;

use std::sync::{Arc, Mutex};

use moq_transport::serve::{ServeError, TracksReader};
use url::Url;

#[derive(Clone)]
pub struct Locals {
	lookup: Arc<Mutex<HashMap<String, TracksReader>>>,
	api: Option<moq_api::Client>,
	node: Option<Url>,
}

impl Locals {
	pub fn new(api: Option<moq_api::Client>, node: Option<Url>) -> Self {
		Self {
			api,
			node,
			lookup: Default::default(),
		}
	}

	pub async fn register(&mut self, tracks: TracksReader) -> anyhow::Result<Registration> {
		let namespace = tracks.namespace.to_string();

		// Try to insert with the API.
		self.store(&namespace).await?;

		let delay = tokio::time::Duration::from_secs(300);
		let mut registration = Registration {
			locals: self.clone(),
			namespace: namespace.to_string(),
			refresh: tokio::time::interval(delay),
		};

		registration.refresh.reset_after(delay); // Skip the first tick

		match self.lookup.lock().unwrap().entry(namespace.to_string()) {
			hash_map::Entry::Vacant(entry) => entry.insert(tracks),
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		Ok(registration)
	}

	async fn unregister(&mut self, namespace: &str) -> anyhow::Result<()> {
		self.lookup
			.lock()
			.unwrap()
			.remove(namespace)
			.ok_or(ServeError::NotFound)?;

		if let Some(api) = self.api.as_ref() {
			api.delete_origin(namespace).await?;
		}

		Ok(())
	}

	async fn store(&mut self, namespace: &str) -> anyhow::Result<()> {
		if let (Some(api), Some(node)) = (self.api.as_ref(), self.node.as_ref()) {
			// Register the origin in moq-api.
			let origin = moq_api::Origin { url: node.clone() };
			log::debug!("registering origin: namespace={} url={}", namespace, node);
			api.set_origin(&namespace, origin).await?;
		}

		Ok(())
	}

	pub fn route(&self, namespace: &str) -> Option<TracksReader> {
		self.lookup.lock().unwrap().get(namespace).cloned()
	}
}

pub struct Registration {
	locals: Locals,
	namespace: String,
	refresh: tokio::time::Interval,
}

impl Registration {
	pub async fn run(mut self) -> anyhow::Result<()> {
		loop {
			self.refresh.tick().await;
			self.locals.store(&self.namespace).await?;
		}
	}
}

impl Drop for Registration {
	fn drop(&mut self) {
		// TODO this is really lazy
		let mut locals = self.locals.clone();
		let namespace = self.namespace.clone();
		tokio::spawn(async move { locals.unregister(&namespace).await });
	}
}
