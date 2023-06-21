use super::track;

use std::collections::hash_map::{Entry, HashMap};
use std::sync::Arc;

use anyhow::Context;
use tokio::sync::{mpsc, Mutex};

pub struct Publisher {
	pub namespace: String,
	requested: mpsc::Receiver<track::Publisher>,
}

#[derive(Clone)]
pub struct Subscriber {
	pub namespace: String,

	tracks: Arc<Mutex<HashMap<String, track::Subscriber>>>,
	requested: mpsc::Sender<track::Publisher>,
}

pub fn new(namespace: String) -> (Publisher, Subscriber) {
	let (tx, rx) = mpsc::channel(16);

	let publisher = Publisher {
		namespace,
		requested: rx,
	};

	let subscriber = Subscriber {
		namespace,
		tracks: Default::default(),
		requested: tx,
	};

	(publisher, subscriber)
}

impl Publisher {
	pub async fn requested(&mut self) -> anyhow::Result<track::Publisher> {
		self.requested.recv().await.context("channel closed")
	}
}

impl Subscriber {
	pub async fn subscribe(&self, name: String) -> anyhow::Result<track::Subscriber> {
		let mut tracks = self.tracks.lock().await;

		let existing = match tracks.entry(name.clone()) {
			Entry::Occupied(o) => return Ok(o.get().clone()),
			Entry::Vacant(v) => v,
		};

		let (publisher, subscriber) = track::new(name.clone());

		tracks.insert(name, subscriber.clone());

		drop(tracks); // release the mutex

		// TODO use the async send, which means we need to figure out the mutex situation.
		self.requested.send(publisher).await.context("publisher dropped")?;

		Ok(subscriber)
	}
}
