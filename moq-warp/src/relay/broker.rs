use crate::model::{track, watch};

use std::collections::hash_map::{Entry, HashMap};
use std::fmt;
use std::sync::{Arc, Mutex};

use tokio::sync::{mpsc, oneshot};

use anyhow::Context;

#[derive(Clone, Default)]
pub struct Broadcasts {
	// Operate on the inner struct so we can share/clone the outer struct.
	inner: Arc<Mutex<BroadcastsInner>>,
}

#[derive(Default)]
struct BroadcastsInner {
	lookup: HashMap<String, Subscriber>,
	updates: watch::Publisher<Update>,
}

#[derive(Clone)]
pub enum Update {
	// Broadcast was announced
	Insert(String),

	// Broadcast was unannounced
	Remove(String),
}

impl Broadcasts {
	pub fn new() -> Self {
		Default::default()
	}

	// Return the list of available broadcasts, and a subscriber that will return updates (add/remove).
	pub fn available(&self) -> (Vec<String>, watch::Subscriber<Update>) {
		// Grab the lock.
		let this = self.inner.lock().unwrap();

		// Get the list of all available tracks.
		let keys = this.lookup.keys().cloned().collect();

		// Get a subscriber that will return updates.
		let updates = this.updates.subscribe();

		(keys, updates)
	}

	pub fn publish(&self, namespace: &str) -> anyhow::Result<Publisher> {
		let mut this = self.inner.lock().unwrap();

		let entry = match this.lookup.entry(namespace.into()) {
			Entry::Occupied(_) => anyhow::bail!("namespace already registered"),
			Entry::Vacant(entry) => entry,
		};

		let (tx, rx) = mpsc::channel(16);

		// Wrapper that automatically calls deregister on drop
		let publisher = Publisher {
			broker: self.clone(),
			namespace: namespace.to_string(),
			requests: rx,
		};

		let subscriber = Subscriber {
			namespace: namespace.to_string(),
			chan: tx,
		};

		entry.insert(subscriber);

		Ok(publisher)
	}

	pub fn subscribe(&self, namespace: &str) -> anyhow::Result<Subscriber> {
		let this = self.inner.lock().unwrap();
		let subscriber = this.lookup.get(namespace).context("failed to find namespace")?.clone();
		Ok(subscriber)
	}

	// Called automatically on drop
	fn unpublish(&self, namespace: &str) {
		let mut this = self.inner.lock().unwrap();
		this.lookup.remove(namespace).expect("namespace was not published");
	}
}

pub struct Publisher {
	broker: Broadcasts,

	pub namespace: String,
	requests: mpsc::Receiver<Request>,
}

impl Publisher {
	pub async fn request(&mut self) -> Option<Request> {
		self.requests.recv().await
	}
}

impl Drop for Publisher {
	fn drop(&mut self) {
		self.broker.unpublish(&self.namespace);
	}
}

// Ask for a track, providing a oneshot channel to receive the result.
pub struct Request {
	pub name: String,
	respond: oneshot::Sender<Response>,
}

impl Request {
	pub fn respond(self, response: Response) {
		self.respond.send(response).ok();
	}
}

impl fmt::Debug for Request {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Request").field("name", &self.name).finish()
	}
}

// Get a response back, with a subscriber object we can poll for segments.
pub type Response = anyhow::Result<track::Subscriber>;

#[derive(Clone)]
pub struct Subscriber {
	pub namespace: String,
	chan: mpsc::Sender<Request>,
}

impl Subscriber {
	// Get the track by name.
	pub async fn track(&self, name: &str) -> Response {
		// Create a request for the publisher to respond to
		let (tx, rx) = oneshot::channel();

		let request = Request {
			name: name.into(),
			respond: tx,
		};

		self.chan.send(request).await.context("publisher was dropped")?;
		rx.await.context("failed to read response")?
	}
}
