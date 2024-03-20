//! A broadcast is a collection of tracks, split into two handles: [Publisher] and [Subscriber].
//!
//! The [Publisher] can create tracks, either manually or on request.
//! It receives all requests by a [Subscriber] for a tracks that don't exist.
//! The simplest implementation is to close every unknown track with [ServeError::NotFound].
//!
//! A [Subscriber] can request tracks by name.
//! If the track already exists, it will be returned.
//! If the track doesn't exist, it will be sent to [Unknown] to be handled.
//! A [Subscriber] can be cloned to create multiple subscriptions.
//!
//! The broadcast is automatically closed with [ServeError::Done] when [Publisher] is dropped, or all [Subscriber]s are dropped.
use std::{
	collections::{hash_map, HashMap},
	fmt,
	ops::Deref,
	sync::Arc,
};

use super::{ServeError, Track, TrackPublisher, TrackSubscriber};
use crate::util::Watch;

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Broadcast {
	pub namespace: String,
}

impl Broadcast {
	pub fn new(namespace: &str) -> Self {
		Self {
			namespace: namespace.to_owned(),
		}
	}

	pub fn produce(self) -> (BroadcastPublisher, BroadcastSubscriber) {
		let state = Watch::new(State::default());
		let info = Arc::new(self);

		let publisher = BroadcastPublisher::new(state.clone(), info.clone());
		let subscriber = BroadcastSubscriber::new(state, info);

		(publisher, subscriber)
	}
}

/// Dynamic information about the broadcast.
#[derive(Debug)]
struct State {
	tracks: HashMap<String, TrackSubscriber>,
	closed: Result<(), ServeError>,
}

impl State {
	pub fn get(&self, name: &str) -> Result<Option<TrackSubscriber>, ServeError> {
		match self.tracks.get(name) {
			Some(track) => Ok(Some(track.clone())),
			// Return any error if we couldn't find a track.
			None => self.closed.clone().map(|_| None),
		}
	}

	pub fn insert(&mut self, track: TrackSubscriber) -> Result<(), ServeError> {
		match self.tracks.entry(track.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(track),
		};

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> Option<TrackSubscriber> {
		self.tracks.remove(name)
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			closed: Ok(()),
		}
	}
}

/// Publish new tracks for a broadcast by name.
#[derive(Debug)]
pub struct BroadcastPublisher {
	state: Watch<State>,
	info: Arc<Broadcast>,
}

impl BroadcastPublisher {
	fn new(state: Watch<State>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	/// Create a new track with the given name, inserting it into the broadcast.
	pub fn create_track(&mut self, track: &str) -> Result<TrackPublisher, ServeError> {
		let (publisher, subscriber) = Track {
			namespace: self.namespace.clone(),
			name: track.to_owned(),
		}
		.produce();

		self.state.lock_mut().insert(subscriber)?;
		Ok(publisher)
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackSubscriber> {
		self.state.lock_mut().remove(track)
	}

	/// Close the broadcast with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for BroadcastPublisher {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone, Debug)]
pub struct BroadcastSubscriber {
	state: Watch<State>,
	info: Arc<Broadcast>,
	_dropped: Arc<Dropped>,
}

impl BroadcastSubscriber {
	fn new(state: Watch<State>, info: Arc<Broadcast>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	/// Get a track from the broadcast by name.
	pub fn get_track(&self, name: &str) -> Result<Option<TrackSubscriber>, ServeError> {
		self.state.lock().get(name)
	}

	/// Wait until if the broadcast is closed, either because the publisher was dropped or called [Publisher::close].
	pub async fn closed(&self) -> ServeError {
		loop {
			let notify = {
				let state = self.state.lock();
				if let Some(err) = state.closed.as_ref().err() {
					return err.clone();
				}

				state.changed()
			};

			notify.await;
		}
	}
}

impl Deref for BroadcastSubscriber {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

struct Dropped {
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for Dropped {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Dropped").finish()
	}
}
