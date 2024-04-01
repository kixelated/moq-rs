//! A broadcast is a collection of tracks, split into two handles: [Writer] and [Reader].
//!
//! The [Writer] can create tracks, either manually or on request.
//! It receives all requests by a [Reader] for a tracks that don't exist.
//! The simplest implementation is to close every unknown track with [ServeError::NotFound].
//!
//! A [Reader] can request tracks by name.
//! If the track already exists, it will be returned.
//! If the track doesn't exist, it will be sent to [Unknown] to be handled.
//! A [Reader] can be cloned to create multiple subscriptions.
//!
//! The broadcast is automatically closed with [ServeError::Done] when [Writer] is dropped, or all [Reader]s are dropped.
use std::{
	collections::{hash_map, HashMap},
	ops::Deref,
	sync::Arc,
};

use super::{ServeError, Track, TrackReader, TrackWriter};
use crate::util::State;

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

	pub fn produce(self) -> (BroadcastWriter, BroadcastReader) {
		let (send, recv) = State::init();
		let info = Arc::new(self);

		let writer = BroadcastWriter::new(send, info.clone());
		let reader = BroadcastReader::new(recv, info);

		(writer, reader)
	}
}

/// Dynamic information about the broadcast.
struct BroadcastState {
	tracks: HashMap<String, TrackReader>,
	closed: Result<(), ServeError>,
}

impl BroadcastState {
	pub fn get(&self, name: &str) -> Result<Option<TrackReader>, ServeError> {
		match self.tracks.get(name) {
			Some(track) => Ok(Some(track.clone())),
			// Return any error if we couldn't find a track.
			None => self.closed.clone().map(|_| None),
		}
	}

	pub fn insert(&mut self, track: TrackReader) -> Result<(), ServeError> {
		match self.tracks.entry(track.name.clone()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(track),
		};

		Ok(())
	}

	pub fn remove(&mut self, name: &str) -> Option<TrackReader> {
		self.tracks.remove(name)
	}
}

impl Default for BroadcastState {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			closed: Ok(()),
		}
	}
}

/// Publish new tracks for a broadcast by name.
pub struct BroadcastWriter {
	state: State<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastWriter {
	fn new(state: State<BroadcastState>, broadcast: Arc<Broadcast>) -> Self {
		Self { state, info: broadcast }
	}

	/// Create a new track with the given name, inserting it into the broadcast.
	pub fn create_track(&mut self, track: &str) -> Result<TrackWriter, ServeError> {
		let (writer, reader) = Track {
			namespace: self.namespace.clone(),
			name: track.to_owned(),
		}
		.produce();

		self.state.lock_mut().ok_or(ServeError::Cancel)?.insert(reader)?;

		Ok(writer)
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackReader> {
		self.state.lock_mut()?.remove(track)
	}

	/// Close the broadcast with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		if let Some(mut state) = state.into_mut() {
			state.closed = Err(err);
		}

		Ok(())
	}
}

impl Deref for BroadcastWriter {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct BroadcastReader {
	state: State<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastReader {
	fn new(state: State<BroadcastState>, broadcast: Arc<Broadcast>) -> Self {
		Self { state, info: broadcast }
	}

	/// Get a track from the broadcast by name.
	pub fn get_track(&self, name: &str) -> Result<Option<TrackReader>, ServeError> {
		self.state.lock().get(name)
	}
}

impl Deref for BroadcastReader {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
