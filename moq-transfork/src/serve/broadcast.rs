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
use std::{collections::HashMap, ops::Deref, sync::Arc};

use super::{ServeError, Track, TrackReader, TrackWriter};
use crate::util::{Queue, State};

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Broadcast {
	pub name: String,
}

impl Broadcast {
	pub fn new(name: String) -> Self {
		Self { name }
	}

	pub fn produce(self) -> (BroadcastWriter, BroadcastRequest, BroadcastReader) {
		let info = Arc::new(self);
		let state = State::default();
		let state_w = state.split(); // both writers share the state split
		let queue = Queue::default();

		let writer = BroadcastWriter::new(state_w.clone(), info.clone());
		let request = BroadcastRequest::new(state_w, queue.split(), info.clone());
		let reader = BroadcastReader::new(state, queue, info);

		(writer, request, reader)
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

	/// Create a new track with the given name, inserting it into the broadcast.
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

pub struct BroadcastRequest {
	#[allow(dead_code)] // Avoid dropping the write side
	state: State<BroadcastState>,
	incoming: Queue<TrackWriter>,
	pub info: Arc<Broadcast>,
}

impl BroadcastRequest {
	fn new(state: State<BroadcastState>, incoming: Queue<TrackWriter>, info: Arc<Broadcast>) -> Self {
		Self { state, incoming, info }
	}

	/// Wait for a request to create a new track.
	/// None is returned if all [BroadcastReader]s have been dropped.
	pub async fn next(&mut self) -> Option<TrackWriter> {
		self.incoming.pop().await
	}
}

impl Deref for BroadcastRequest {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl Drop for BroadcastRequest {
	fn drop(&mut self) {
		// Close any tracks still in the Queue
		for mut track in self.incoming.drain() {
			let _ = track.close(ServeError::NotFound);
		}
	}
}

/// Subscribe to a broadcast by requesting tracks.
///
/// This can be cloned to create handles.
#[derive(Clone)]
pub struct BroadcastReader {
	state: State<BroadcastState>,
	queue: Queue<TrackWriter>,
	pub info: Arc<Broadcast>,
}

impl BroadcastReader {
	fn new(state: State<BroadcastState>, queue: Queue<TrackWriter>, info: Arc<Broadcast>) -> Self {
		Self { state, queue, info }
	}

	/// Get or request a track from the broadcast by name.
	/// None is returned if [BroadcastWriter] or [BroadcastRequest] cannot fufill the request.
	pub fn subscribe(&mut self, name: &str) -> Option<TrackReader> {
		let state = self.state.lock();

		if let Some(reader) = state.tracks.get(name).cloned() {
			return Some(reader);
		}

		let mut state = state.into_mut()?;
		let track = Track::new(name).build();
		let (writer, reader) = track.produce();

		if self.queue.push(writer).is_err() {
			return None;
		}

		// We requested the track sucessfully so we can deduplicate it.
		state.tracks.insert(reader.name.clone(), reader.clone());

		Some(reader)
	}
}

impl Deref for BroadcastReader {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
