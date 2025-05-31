use std::{
	collections::HashMap,
	future::Future,
	sync::{
		atomic::{AtomicUsize, Ordering},
		Arc,
	},
};

use crate::{Error, TrackConsumer, TrackProducer};
use tokio::sync::watch;
use web_async::Lock;

use super::Track;

type State = HashMap<String, TrackConsumer>;

/// Receive broadcast/track requests and return if we can fulfill them.
pub struct BroadcastProducer {
	published: Lock<State>,
	closed: watch::Sender<bool>,
	requested: (
		async_channel::Sender<TrackProducer>,
		async_channel::Receiver<TrackProducer>,
	),
	cloned: Arc<AtomicUsize>,
}

impl BroadcastProducer {
	pub fn new() -> Self {
		Self {
			published: Default::default(),
			closed: Default::default(),
			requested: async_channel::unbounded(),
			cloned: Default::default(),
		}
	}

	pub async fn request(&mut self) -> Option<TrackProducer> {
		let track = self.requested.1.recv().await.ok()?;
		web_async::spawn(Self::cleanup(track.consume(), self.published.clone()));
		Some(track)
	}

	pub fn create(&mut self, track: Track) -> TrackProducer {
		let producer = track.produce();
		self.insert(producer.consume());
		producer
	}

	/// Insert a track into the lookup, returning true if it was unique.
	pub fn insert(&mut self, track: TrackConsumer) -> bool {
		let unique = self
			.published
			.lock()
			.insert(track.info.name.clone(), track.clone())
			.is_none();

		web_async::spawn(Self::cleanup(track, self.published.clone()));

		unique
	}

	// Remove the track from the lookup when it's closed.
	async fn cleanup(track: TrackConsumer, published: Lock<State>) {
		// Wait until the track is closed and remove it from the lookup.
		track.closed().await.ok();

		// Remove the track from the lookup.
		let mut published = published.lock();
		match published.remove(&track.info.name) {
			// Make sure we are removing the correct track.
			Some(track) if track.is_clone(&track) => true,
			// Put it back if it's not the same track.
			Some(other) => published.insert(track.info.name.clone(), other).is_some(),
			None => false,
		};
	}

	// Try to create a new consumer.
	pub fn consume(&self) -> BroadcastConsumer {
		BroadcastConsumer {
			published: self.published.clone(),
			closed: self.closed.subscribe(),
			requested: self.requested.0.clone(),
		}
	}

	pub fn finish(&mut self) {
		self.closed.send_modify(|closed| *closed = true);
	}

	/// Block until there are no more consumers.
	///
	/// A new consumer can be created by calling [Self::consume] and this will block again.
	pub fn unused(&self) -> impl Future<Output = ()> {
		let closed = self.closed.clone();
		async move { closed.closed().await }
	}

	pub fn is_clone(&self, other: &Self) -> bool {
		self.closed.same_channel(&other.closed)
	}
}

impl Clone for BroadcastProducer {
	fn clone(&self) -> Self {
		self.cloned.fetch_add(1, Ordering::Relaxed);
		Self {
			published: self.published.clone(),
			closed: self.closed.clone(),
			requested: self.requested.clone(),
			cloned: self.cloned.clone(),
		}
	}
}

impl Drop for BroadcastProducer {
	fn drop(&mut self) {
		if self.cloned.fetch_sub(1, Ordering::Relaxed) > 0 {
			return;
		}

		// Cleanup any lingering state when the last producer is dropped.

		// Close the sender so consumers can't send any more requests.
		self.requested.0.close();

		// Drain any remaining requests.
		while let Ok(producer) = self.requested.1.try_recv() {
			producer.abort(Error::Cancel);
		}

		// Cleanup any published tracks.
		self.published.lock().clear();
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

	pub fn assert_request(&mut self) -> TrackProducer {
		self.request()
			.now_or_never()
			.expect("should not have blocked")
			.expect("should be a request")
	}

	pub fn assert_no_request(&mut self) {
		assert!(self.request().now_or_never().is_none(), "should have blocked");
	}
}

/// Subscribe to abitrary broadcast/tracks.
#[derive(Clone)]
pub struct BroadcastConsumer {
	published: Lock<State>,
	closed: watch::Receiver<bool>,
	requested: async_channel::Sender<TrackProducer>,
}

impl BroadcastConsumer {
	pub fn subscribe(&self, track: &Track) -> TrackConsumer {
		/*
		let closed = match self.closed.wait_for(|closed| *closed).now_or_never() {
			None => false, // would have blocked
			Some(true) => true,
			Some(false) => false,
		};

		if closed {
			// Kind of hacky, but return a closed track consumer.
			let track = track.clone().produce();
			track.abort(Error::Cancel);
			return track.consume();
		}
		*/

		let mut published = self.published.lock();

		// Return any explictly published track.
		if let Some(consumer) = published.get(&track.name).cloned() {
			return consumer;
		}

		// Otherwise we have never seen this track before and need to create a new producer.
		let producer = track.clone().produce();
		let consumer = producer.consume();
		published.insert(track.name.clone(), consumer.clone());

		// Insert the producer into the lookup so we will deduplicate requests.
		// This is not a subscriber so it doesn't count towards "used" subscribers.
		match self.requested.try_send(producer) {
			Ok(()) => {}
			Err(error) => error.into_inner().abort(Error::Cancel),
		}

		consumer
	}

	pub fn closed(&self) -> impl Future<Output = ()> {
		// A hacky way to check if the broadcast is closed.
		let mut closed = self.closed.clone();
		async move {
			closed.wait_for(|closed| *closed).await.ok();
		}
	}

	/// Check if this is the exact same instance of a broadcast.
	///
	/// Duplicate names are allowed in the case of resumption.
	pub fn is_clone(&self, other: &Self) -> bool {
		self.closed.same_channel(&other.closed)
	}
}

#[cfg(test)]
impl BroadcastConsumer {
	pub fn assert_not_closed(&self) {
		assert!(self.closed().now_or_never().is_none(), "should not be closed");
	}

	pub fn assert_closed(&self) {
		assert!(self.closed().now_or_never().is_some(), "should be closed");
	}
}

#[cfg(test)]
mod test {
	use super::*;

	#[tokio::test]
	async fn insert() {
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
		consumer.assert_not_closed();

		// Create a new track and insert it into the broadcast.
		let mut track1 = Track::new("track1").produce();
		track1.append_group();
		producer.insert(track1.consume());

		let mut track1c = consumer.subscribe(&track1.info);
		let track2 = consumer.subscribe(&Track::new("track2"));

		drop(producer);
		consumer.assert_closed();

		// The requested TrackProducer should have been dropped, so the track should be closed.
		track2.assert_closed();

		// But track1 is still open because we currently don't cascade the closed state.
		track1c.assert_group();
		track1c.assert_no_group();
		track1c.assert_not_closed();

		// TODO: We should probably cascade the closed state.
		drop(track1);
		track1c.assert_closed();
	}

	#[tokio::test]
	async fn select() {
		let mut producer = BroadcastProducer::new();

		// Make sure this compiles; it's actually more involved than it should be.
		tokio::select! {
			_ = producer.unused() => {}
			_ = producer.request() => {}
		}
	}

	#[tokio::test]
	async fn requests() {
		let mut producer = BroadcastProducer::new();

		let consumer = producer.consume();
		let consumer2 = consumer.clone();

		let mut track1 = consumer.subscribe(&Track::new("track1"));
		track1.assert_not_closed();
		track1.assert_no_group();

		// Make sure we deduplicate requests.
		let mut track2 = consumer2.subscribe(&Track::new("track1"));
		track2.assert_is_clone(&track1);

		// Get the requested track, and there should only be one.
		let mut track3 = producer.assert_request();
		producer.assert_no_request();

		// Make sure the consumer is the same.
		track3.consume().assert_is_clone(&track1);

		// Append a group and make sure they all get it.
		track3.append_group();
		track1.assert_group();
		track2.assert_group();

		// Make sure that tracks are cancelled when the producer is dropped.
		let track4 = consumer.subscribe(&Track::new("track2"));
		drop(producer);

		// Make sure the track is errored, not closed.
		track4.assert_error();

		let track5 = consumer2.subscribe(&Track::new("track3"));
		track5.assert_error();
	}
}
