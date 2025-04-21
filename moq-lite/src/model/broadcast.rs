use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use crate::{TrackConsumer, TrackProducer};
use tokio::sync::watch;

use super::Track;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Broadcast {
	pub path: String,
}

impl Broadcast {
	pub fn new<T: ToString>(path: T) -> Self {
		Self { path: path.to_string() }
	}

	pub fn produce(self) -> BroadcastProducer {
		BroadcastProducer::new(self)
	}

	/// Return a new broadcast with the given prefix removed from the path.
	///
	/// If the prefix is not a prefix of the path, return None.
	pub fn strip_prefix(self, prefix: &str) -> Option<Broadcast> {
		if prefix.is_empty() {
			Some(self)
		} else {
			let suffix = self.path.strip_prefix(prefix)?;
			Some(suffix.into())
		}
	}
}

impl<T: ToString> From<T> for Broadcast {
	fn from(path: T) -> Self {
		Self::new(path)
	}
}

/// Receive broadcast/track requests and return if we can fulfill them.
///
/// This is a pull-based producer.
/// If you want an easier push-based producer, use [BroadcastProducer::map].
#[derive(Clone)]
pub struct BroadcastProducer {
	pub info: Broadcast,

	lookup: Arc<Mutex<HashMap<String, TrackConsumer>>>,
	queue: async_channel::Receiver<TrackProducer>,
	weak: async_channel::Sender<TrackProducer>,

	// Dropped when all senders or all receivers are dropped.
	// TODO Make a better way of doing this.
	closed: watch::Sender<()>,
}

impl BroadcastProducer {
	pub fn new(info: Broadcast) -> Self {
		let (send, recv) = async_channel::bounded(32);

		Self {
			info,
			queue: recv,
			lookup: Arc::new(Mutex::new(HashMap::new())),
			weak: send.clone(),
			closed: watch::Sender::default(),
		}
	}

	pub async fn requested(&self) -> TrackProducer {
		self.queue.recv().await.unwrap()
	}

	pub fn create(&self, track: Track) -> TrackProducer {
		let producer = track.produce();
		self.insert(producer.consume());
		producer
	}

	/// Insert a new track into the lookup, returning the old track if it already exists.
	pub fn insert(&self, track: TrackConsumer) -> Option<TrackConsumer> {
		let mut lookup = self.lookup.lock().unwrap();
		lookup.insert(track.info.name.clone(), track)
	}

	/// Remove a track from the lookup.
	pub fn remove(&self, name: &str) -> Option<TrackConsumer> {
		let mut lookup = self.lookup.lock().unwrap();
		lookup.remove(name)
	}

	// Try to create a new consumer.
	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			info: self.info.clone(),
			queue: self.weak.clone(),
			lookup: self.lookup.clone(),
			closed: self.closed.subscribe(),
		}
	}

	/// Block until there are no more consumers.
	///
	/// A new consumer can be created by calling [Self::consume] and this will block again.
	pub async fn unused(&self) {
		self.closed.closed().await;
	}
}

impl From<Broadcast> for BroadcastProducer {
	fn from(info: Broadcast) -> Self {
		BroadcastProducer::new(info)
	}
}

/// Subscribe to abitrary broadcast/tracks.
#[derive(Clone)]
pub struct BroadcastConsumer {
	pub info: Broadcast,

	lookup: Arc<Mutex<HashMap<String, TrackConsumer>>>,
	queue: async_channel::Sender<TrackProducer>,

	// Annoying, but we need to know when the above channel is closed without sending.
	closed: watch::Receiver<()>,
}

impl BroadcastConsumer {
	pub fn subscribe(&self, track: Track) -> TrackConsumer {
		if let Some(consumer) = self.lookup.lock().unwrap().get(&track.name).cloned() {
			return consumer;
		}

		let producer = track.produce();
		let consumer = producer.consume();

		let queue = self.queue.clone();
		web_async::spawn(async move {
			let _ = queue.send(producer).await;
		});

		consumer
	}

	pub async fn closed(&self) {
		self.closed.clone().changed().await.ok();
	}
}
