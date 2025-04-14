use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use crate::{Error, TrackConsumer, TrackProducer};
use tokio::sync::{oneshot, watch};

use super::Track;

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Broadcast {
	pub path: String,
}

impl Broadcast {
	pub fn new<P: ToString>(path: P) -> Self {
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
			Some(Broadcast::new(suffix))
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
	queue: async_channel::Receiver<BroadcastRequest>,
	weak: async_channel::Sender<BroadcastRequest>,

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
			weak: send.clone(),
			closed: watch::Sender::default(),
		}
	}

	pub async fn requested(&self) -> BroadcastRequest {
		self.queue.recv().await.unwrap()
	}

	// Try to create a new consumer.
	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			info: self.info.clone(),
			queue: self.weak.clone(),
			closed: self.closed.subscribe(),
		}
	}

	/// Block until there are no more consumers.
	///
	/// A new consumer can be created by calling [Self::consume] and this will block again.
	pub async fn unused(&self) {
		self.closed.closed().await;
	}

	// TODO Block until there is at least one consumer.
	//pub async fn used(&self) {
	//}

	pub fn map(self) -> BroadcastMap {
		let map = BroadcastMap::new(self);
		web_async::spawn(map.clone().serve());
		map
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
	queue: async_channel::Sender<BroadcastRequest>,

	// Annoying, but we need to know when the above channel is closed without sending.
	closed: watch::Receiver<()>,
}

impl BroadcastConsumer {
	pub async fn request(&self, track: Track) -> Result<TrackConsumer, Error> {
		let (send, recv) = oneshot::channel();
		let request = BroadcastRequest { track, reply: send };

		if self.queue.send(request).await.is_err() {
			return Err(Error::Cancel);
		}

		recv.await.map_err(|_| Error::Cancel)?
	}

	pub async fn closed(&self) {
		self.closed.clone().changed().await.ok();
	}
}

#[derive(Clone)]
pub struct BroadcastMap {
	pub inner: BroadcastProducer,
	lookup: Arc<Mutex<HashMap<String, TrackConsumer>>>,
}

impl BroadcastMap {
	pub fn new(inner: BroadcastProducer) -> Self {
		Self {
			lookup: Arc::new(Mutex::new(HashMap::new())),
			inner,
		}
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

	pub fn consume(&self) -> BroadcastConsumer {
		self.inner.consume()
	}

	// Convert a push-based producer into a pull-based producer.
	async fn serve(self) {
		loop {
			let request = self.inner.requested().await;

			let lookup = self.lookup.lock().unwrap();
			if let Some(consumer) = lookup.get(&request.track.name) {
				request.serve(consumer.clone());
			} else {
				request.close(Error::NotFound);
			}
		}
	}
}

/// An outstanding request for a path.
pub struct BroadcastRequest {
	pub track: Track,
	reply: oneshot::Sender<Result<TrackConsumer, Error>>,
}

impl BroadcastRequest {
	pub fn serve(self, reader: TrackConsumer) {
		self.reply.send(Ok(reader)).ok();
	}

	pub fn produce(self) -> TrackProducer {
		let track = self.track.produce();
		self.reply.send(Ok(track.consume())).ok();
		track
	}

	pub fn close(self, error: Error) {
		self.reply.send(Err(error)).ok();
	}
}
