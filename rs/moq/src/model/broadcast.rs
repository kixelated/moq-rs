use std::{
	collections::{BTreeMap, HashMap},
	future::Future,
};

use crate::{TrackConsumer, TrackProducer};
use tokio::sync::watch;

use super::Track;

#[derive(Default)]
struct PublisherState {
	tracks: HashMap<String, TrackConsumer>,

	// NOTE: Not a Result because there's currently no way to announce a broadcast error.
	closed: bool,
}

type ConsumerState = BTreeMap<String, TrackProducer>;

/// Receive broadcast/track requests and return if we can fulfill them.
#[derive(Clone)]
pub struct BroadcastProducer {
	published: watch::Sender<PublisherState>,
	// Unfortunately, this needs to be a sender so we can pop requests.
	requested: watch::Sender<ConsumerState>,
}

impl BroadcastProducer {
	pub fn new() -> Self {
		let this = Self {
			published: Default::default(),
			requested: Default::default(),
		};

		let mut published = this.published.subscribe();
		let requested = this.requested.clone();

		// Unfortunately, we should spawn a task to clean up any requested tracks that haven't been fulfilled.
		// Otherwise they will only get cleared if all producers AND consumers are dropped.
		web_async::spawn(async move {
			published.wait_for(|published| published.closed).await.ok();
			requested.send_modify(|requested| requested.clear());
		});

		this
	}

	pub async fn requested(&mut self) -> Option<TrackProducer> {
		let mut requested = self.requested.subscribe();
		loop {
			// Wait until there is at least one request.
			requested.wait_for(|requested| !requested.is_empty()).await.ok()?;

			// Unfortunately, we can't consume using the read-only receiver.
			// So we need to switch to the mutable sender to pop the state.
			let mut result = None;
			self.requested.send_modify(|requested| {
				result = requested.pop_first();
			});

			// This is racey so we may need to loop again.
			if let Some((name, producer)) = result {
				// Save the consumer so we can deduplicate requests.
				self.published.send_modify(|published| {
					published.tracks.insert(name, producer.consume());
				});
				return Some(producer);
			}
		}
	}

	pub fn create(&mut self, track: Track) -> TrackProducer {
		let producer = track.produce();
		self.insert(producer.consume());
		producer
	}

	/// Insert a track into the lookup, returning true if it was unique.
	pub fn insert(&mut self, track: TrackConsumer) -> bool {
		let unique = self.published.send_if_modified(|published| {
			published
				.tracks
				.insert(track.info.name.clone(), track.clone())
				.is_none()
		});

		let published = self.published.clone();

		web_async::spawn(async move {
			// Wait until the track is closed and remove it from the lookup.
			track.closed().await.ok();

			// Remove the track from the lookup.
			published.send_if_modified(|published| {
				match published.tracks.remove(&track.info.name) {
					// Make sure we are removing the correct track.
					Some(track) if track.is_clone(&track) => true,
					// Put it back if it's not the same track.
					Some(other) => published.tracks.insert(track.info.name.clone(), other).is_some(),
					None => false,
				}
			});
		});

		unique
	}

	/*
	/// Remove a track from the lookup, returning true if it was removed.
	pub fn remove(&mut self, name: &str) -> bool {
		self.state.send_if_modified(|state| {
			state.published.remove(name);
			false
		})
	}
	*/

	// Try to create a new consumer.
	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			published: self.published.subscribe(),
			requested: self.requested.clone(),
		}
	}

	pub fn finish(&mut self) {
		self.published.send_modify(|published| published.closed = true);
	}

	/// Block until there are no more consumers.
	///
	/// A new consumer can be created by calling [Self::consume] and this will block again.
	pub fn unused(&self) -> impl Future<Output = ()> {
		let published = self.published.clone();
		async move {
			published.closed().await;
		}
	}

	pub fn is_clone(&self, other: &Self) -> bool {
		self.requested.same_channel(&other.requested)
	}
}

#[cfg(test)]
use futures::FutureExt;

#[cfg(test)]
impl BroadcastProducer {
	pub fn assert_used(&self) {
		assert!(self.unused().now_or_never().is_none(), "should be used");
	}

	pub fn assert_unused(&self) {
		assert!(self.unused().now_or_never().is_some(), "should be unused");
	}
}

/// Subscribe to abitrary broadcast/tracks.
#[derive(Clone)]
pub struct BroadcastConsumer {
	published: watch::Receiver<PublisherState>,
	requested: watch::Sender<ConsumerState>,
}

impl BroadcastConsumer {
	pub fn subscribe(&self, track: &Track) -> TrackConsumer {
		let published = self.published.borrow();

		if published.closed {
			// Kind of hacky, but return a closed track consumer.
			return track.clone().produce().consume();
		}

		// Return any explictly published track.
		if let Some(consumer) = published.tracks.get(&track.name).cloned() {
			return consumer;
		}

		let mut consumer = None;

		self.requested.send_if_modified(|requested| {
			// Deduplicate any requested track.
			if let Some(requested) = requested.get(&track.name) {
				consumer = Some(requested.consume());
				return false;
			}

			// Otherwise we have never seen this track before and need to create a new producer.
			// Otherwise we have never seen this track before and need to create a new producer.
			let producer = track.clone().produce();
			consumer = Some(producer.consume());

			// Insert the producer into the lookup so we will deduplicate requests.
			// This is not a subscriber so it doesn't count towards "used" subscribers.
			requested.insert(track.name.clone(), producer.clone());

			true
		});

		consumer.unwrap()
	}

	pub fn closed(&self) -> impl Future<Output = ()> {
		// A hacky way to check if the broadcast is closed.
		let mut published = self.published.clone();
		async move {
			published.wait_for(|published| published.closed).await.ok();
		}
	}

	/// Check if this is the exact same instance of a broadcast.
	///
	/// Duplicate names are allowed in the case of resumption.
	pub fn is_clone(&self, other: &Self) -> bool {
		self.requested.same_channel(&other.requested)
	}
}

#[cfg(test)]
impl BroadcastConsumer {
	pub fn assert_active(&self) {
		assert!(self.closed().now_or_never().is_none(), "should not be closed");
	}

	pub fn assert_closed(&self) {
		assert!(self.closed().now_or_never().is_some(), "should be closed");
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[test]
	fn insert() {
		let mut producer = BroadcastProducer::new();
		let mut track1 = Track::new("track1").produce();

		// Make sure we can insert before a consumer is created.
		producer.insert(track1.consume());
		track1.append_group();

		let consumer = producer.consume();

		let mut track1 = consumer.subscribe(&track1.info);
		track1.assert_group();

		let mut track2 = Track::new("track2").produce();
		producer.insert(track2.consume());

		let consumer2 = producer.consume();
		let mut track2consumer = consumer2.subscribe(&track2.info);
		track2consumer.assert_no_group();

		track2.append_group();

		track2consumer.assert_group();
	}

	#[tokio::test]
	async fn unused() {
		let producer = BroadcastProducer::new();
		producer.assert_unused();

		// Create a new consumer.
		let consumer1 = producer.consume();
		producer.assert_used();

		// It's also valid to clone the consumer.
		let consumer2 = consumer1.clone();
		producer.assert_used();

		// Dropping one consumer doesn't make it unused.
		drop(consumer1);
		producer.assert_used();

		drop(consumer2);
		producer.assert_unused();

		// Even though it's unused, we can still create a new consumer.
		let consumer3 = producer.consume();
		producer.assert_used();

		let track1 = consumer3.subscribe(&Track::new("track1"));

		// It doesn't matter if a subscription is alive, we only care about the broadcast handle.
		// TODO is this the right behavior?
		drop(consumer3);
		producer.assert_unused();

		drop(track1);
	}

	#[tokio::test]
	async fn closed() {
		let mut producer = BroadcastProducer::new();

		let consumer = producer.consume();
		consumer.assert_active();

		// Create a new track and insert it into the broadcast.
		let mut track1 = Track::new("track1").produce();
		track1.append_group();
		producer.insert(track1.consume());

		let mut track1 = consumer.subscribe(&track1.info);
		let mut track2 = consumer.subscribe(&Track::new("track2"));

		drop(producer);
		consumer.assert_closed();

		track1.assert_group(); // debatable
		track1.assert_no_group();
		track2.assert_no_group();
	}

	#[tokio::test]
	async fn select() {
		let mut producer = BroadcastProducer::new();

		// Make sure this compiles.
		tokio::select! {
			_ = producer.unused() => {}
			_ = producer.requested() => {}
		}
	}
}
