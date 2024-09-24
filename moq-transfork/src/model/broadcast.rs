//! A broadcast is a collection of tracks, split into two handles: [Producer] and [Consumer].
//!
//! The [Producer] can create tracks, either manually or on request.
//! It receives all requests by a [Consumer] for a tracks that don't exist.
//! The simplest implementation is to close every unknown track with [ServeError::NotFound].
//!
//! A [Consumer] can request tracks by name.
//! If the track already exists, it will be returned.
//! If the track doesn't exist, it will be sent to [Unknown] to be handled.
//! A [Consumer] can be cloned to create multiple subscriptions.
//!
//! The broadcast is automatically closed with [ServeError::Done] when [Producer] is dropped, or all [Consumer]s are dropped.
use std::{collections::HashMap, ops, sync::Arc, time};

use tokio::sync::watch;

use super::{GroupOrder, Produce, RouterConsumer, Track, TrackBuilder, TrackConsumer, TrackProducer};
use crate::Error;

/// Static information about a broadcast.
#[derive(Clone, PartialEq, Eq, Hash, Debug)]
pub struct Broadcast {
	pub name: String,
}

impl Broadcast {
	pub fn new<T: Into<String>>(name: T) -> Self {
		Self { name: name.into() }
	}
}

impl<T: Into<String>> From<T> for Broadcast {
	fn from(name: T) -> Self {
		Self::new(name)
	}
}

impl Produce for Broadcast {
	type Consumer = BroadcastConsumer;
	type Producer = BroadcastProducer;

	fn produce(self) -> (BroadcastProducer, BroadcastConsumer) {
		let info = Arc::new(self);
		let (send, recv) = watch::channel(BroadcastState::default());

		let writer = BroadcastProducer::new(send, info.clone());
		let reader = BroadcastConsumer::new(recv, info);

		(writer, reader)
	}
}

pub struct BroadcastState {
	tracks: HashMap<String, TrackConsumer>,
	router: Option<RouterConsumer<Track>>,
	closed: Result<(), Error>,
}

impl Default for BroadcastState {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			router: None,
			closed: Ok(()),
		}
	}
}

/// Publish new tracks for a broadcast by name.
pub struct BroadcastProducer {
	state: watch::Sender<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastProducer {
	fn new(state: watch::Sender<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	pub fn build_track<T: Into<String>>(&mut self, name: T) -> BroadcastTrackBuilder {
		BroadcastTrackBuilder::new(self, name.into())
	}

	/// Optionally route requests for unknown tracks.
	pub fn route_tracks(&mut self, router: RouterConsumer<Track>) {
		self.state.send_modify(|state| {
			state.router = Some(router);
		});
	}

	/// Insert a track into the broadcast.
	pub fn insert_track<T: Into<Track>>(&mut self, track: T) -> TrackProducer {
		let (writer, reader) = track.into().produce();

		// NOTE: We overwrite the track if it already exists.
		self.state.send_modify(|state| {
			state.tracks.insert(reader.name.clone(), reader);
		});

		writer
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackConsumer> {
		let mut reader = None;
		self.state.send_if_modified(|state| {
			reader = state.tracks.remove(track);
			reader.is_some()
		});
		reader
	}

	pub fn has_track(&self, track: &str) -> bool {
		self.state.borrow().tracks.contains_key(track)
	}

	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}

	// Returns when there are no references to the consumer
	pub async fn unused(&self) {
		self.state.closed().await
	}

	pub fn is_unused(&self) -> bool {
		!self.state.is_closed()
	}
}

impl ops::Deref for BroadcastProducer {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub struct BroadcastTrackBuilder<'a> {
	broadcast: &'a mut BroadcastProducer,
	track: TrackBuilder,
}

impl<'a> BroadcastTrackBuilder<'a> {
	fn new(broadcast: &'a mut BroadcastProducer, name: String) -> Self {
		Self {
			track: Track::build(name),
			broadcast,
		}
	}

	pub fn priority(mut self, priority: i8) -> Self {
		self.track = self.track.priority(priority);
		self
	}

	pub fn group_order(mut self, order: GroupOrder) -> Self {
		self.track = self.track.group_order(order);
		self
	}

	pub fn group_expires(mut self, expires: time::Duration) -> Self {
		self.track = self.track.group_expires(expires);
		self
	}

	pub fn insert(self) -> TrackProducer {
		self.broadcast.insert_track(self.track)
	}
}

impl<'a> ops::Deref for BroadcastTrackBuilder<'a> {
	type Target = TrackBuilder;

	fn deref(&self) -> &TrackBuilder {
		&self.track
	}
}

impl<'a> ops::DerefMut for BroadcastTrackBuilder<'a> {
	fn deref_mut(&mut self) -> &mut Self::Target {
		&mut self.track
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct BroadcastConsumer {
	state: watch::Receiver<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastConsumer {
	fn new(state: watch::Receiver<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	/// Get a track from the broadcast by name.
	pub async fn subscribe<T: Into<Track>>(&self, track: T) -> Result<TrackConsumer, Error> {
		let track = track.into();

		let router = {
			let state = self.state.borrow();
			if let Some(track) = state.tracks.get(&track.name).cloned() {
				return Ok(track);
			}

			state.router.clone().ok_or(Error::NotFound)?
		};

		// TODO cache to deduplicate?
		router.subscribe(track).await
	}

	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}

impl ops::Deref for BroadcastConsumer {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
