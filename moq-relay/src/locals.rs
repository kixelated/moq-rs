use std::collections::hash_map;
use std::collections::HashMap;

use std::sync::Arc;
use std::sync::Mutex;

use futures::stream::FuturesUnordered;
use futures::FutureExt;
use moq_dir::ListingDelta;
use moq_dir::ListingReader;
use moq_transport::serve::ServeError;
use moq_transport::serve::Track;
use moq_transport::serve::TracksReader;
use moq_transport::session::Publisher;
use moq_transport::session::Subscriber;

#[derive(Clone)]
pub struct Locals {
	announce: Option<Publisher>,

	// A lookup of all local broadcasts.
	local: Arc<Mutex<HashMap<String, TracksReader>>>,

	// A lookup of all remote broadcasts.
	remote: Arc<Mutex<HashMap<String, TracksReader>>>,
}

impl Locals {
	pub fn new(announce: Option<Publisher>) -> Self {
		Self {
			announce,
			local: Default::default(),
			remote: Default::default(),
		}
	}

	pub async fn announce(&mut self, tracks: TracksReader) -> Result<(), ServeError> {
		match self.local.lock().unwrap().entry(tracks.namespace.clone()) {
			hash_map::Entry::Vacant(entry) => entry.insert(tracks.clone()),
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
		};

		if let Some(forward) = self.announce.as_mut() {
			forward.announce(tracks).await?;
		}

		// TODO block indefinitely and remove on Drop
		// TODO remove on Drop

		Ok(())
	}

	pub fn route(&self, namespace: &str) -> Option<TracksReader> {
		if let Some(reader) = self.local.lock().unwrap().get(namespace) {
			return Some(reader.clone());
		}

		None
	}

	async fn unannounce(&mut self, namespace: &str) -> Result<(), ServeError> {
		self.local
			.lock()
			.unwrap()
			.remove(namespace)
			.ok_or(ServeError::NotFound)?;
		Ok(())
	}
}

pub struct Remotes {}

impl Remotes {
	pub fn new() -> Self {
		Self {}
	}

	pub async fn run(&self, mut remote: Subscriber) -> anyhow::Result<()> {
		let (writer, reader) = Track {
			namespace: "/".to_string(),
			name: "node/".to_string(),
		}
		.produce();

		let active = remote.subscribe(writer).boxed();

		let listing = ListingReader::new(reader);

		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				next = listing.next() => match next? {
					ListingDelta::Add(node) => tasks.push(async move {
						self.run_node(node).await
					}),
					ListingDelta::Rem(node) => log::warn!("removing node: {:?}", node),
					ListingDelta::Done => break,
				},
				done = active => return done.map_err(Into::into),
			}
		}
	}

	async fn run_node(&self, node: String) -> anyhow::Result<()> {
		let (writer, reader) = Track {
			namespace: format!("/node/{}", node),
			name: "namespaces".to_string(),
		}
		.produce();
	}
}
