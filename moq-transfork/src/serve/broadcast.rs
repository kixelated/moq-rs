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
	collections::HashMap,
	ops::{self, Deref},
	sync::Arc,
};

use super::{Track, TrackBuilder, TrackReader, TrackWriter};
use crate::util::State;

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Broadcast {
	pub name: String,
}

impl Broadcast {
	pub fn new(name: String) -> Self {
		Self { name }
	}

	pub fn produce(self) -> (BroadcastWriter, BroadcastReader) {
		let info = Arc::new(self);
		let state = State::default();

		let writer = BroadcastWriter::new(state.split(), info.clone());
		let reader = BroadcastReader::new(state, info);

		(writer, reader)
	}
}

#[derive(Default)]
pub struct BroadcastState {
	tracks: HashMap<String, TrackReader>,
}

/// Publish new tracks for a broadcast by name.
pub struct BroadcastWriter {
	state: State<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastWriter {
	fn new(state: State<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	pub fn create_track(&mut self, name: &str) -> BroadcastTrackBuilder {
		BroadcastTrackBuilder::new(self, name)
	}

	/// Insert a track into the broadcast.
	/// None is returned if all [BroadcastReader]s have been dropped.
	pub fn insert_track(&mut self, track: Track) -> Option<TrackWriter> {
		let (writer, reader) = track.produce();

		// NOTE: We overwrite the track if it already exists.
		self.state.lock_mut()?.tracks.insert(reader.name.clone(), reader);

		Some(writer)
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackReader> {
		self.state.lock_mut()?.tracks.remove(track)
	}
}

impl Deref for BroadcastWriter {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub struct BroadcastTrackBuilder<'a> {
	broadcast: &'a mut BroadcastWriter,
	track: TrackBuilder,
}

impl<'a> BroadcastTrackBuilder<'a> {
	fn new(broadcast: &'a mut BroadcastWriter, name: &str) -> Self {
		Self {
			track: Track::new(&broadcast.name, name),
			broadcast,
		}
	}

	/// None is returned if all [BroadcastReader]s have been dropped.
	pub fn build(self) -> Option<TrackWriter> {
		self.broadcast.insert_track(self.track.build())
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
pub struct BroadcastReader {
	state: State<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastReader {
	fn new(state: State<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	/// Get or request a track from the broadcast by name.
	/// None is returned if [BroadcastWriter] or [BroadcastRequest] cannot fufill the request.
	pub fn get_track(&mut self, name: &str) -> Option<TrackReader> {
		let state = self.state.lock();
		state.tracks.get(name).cloned()
	}
}

impl Deref for BroadcastReader {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
