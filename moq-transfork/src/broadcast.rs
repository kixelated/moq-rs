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

use super::{Track, TrackBuilder, TrackReader, TrackWriter, UnknownReader};
use crate::{util::State, ServeError};

/// Static information about a broadcast.
#[derive(Debug)]
pub struct Broadcast {
	pub name: String,
}

impl Broadcast {
	pub fn new(name: &str) -> Self {
		Self { name: name.to_string() }
	}

	pub fn produce(self) -> (BroadcastWriter, BroadcastReader) {
		let info = Arc::new(self);
		let state = State::default();

		let writer = BroadcastWriter::new(state.split(), info.clone());
		let reader = BroadcastReader::new(state, info);

		(writer, reader)
	}
}

pub struct BroadcastState {
	tracks: HashMap<String, TrackReader>,
	unknown: Option<UnknownReader>,
	closed: Result<(), ServeError>,
}

impl Default for BroadcastState {
	fn default() -> Self {
		Self {
			tracks: HashMap::new(),
			unknown: None,
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
	fn new(state: State<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	pub fn create(&mut self, name: &str) -> BroadcastTrackBuilder {
		BroadcastTrackBuilder::new(self, name)
	}

	/// Optionally route unknown tracks to the provided [UnknownReader].
	pub fn unknown(&mut self, reader: UnknownReader) {
		if let Some(mut state) = self.state.lock_mut() {
			state.unknown = Some(reader);
		}
	}

	/// Insert a track into the broadcast.
	/// None is returned if all [BroadcastReader]s have been dropped.
	pub fn insert(&mut self, track: Track) -> Option<TrackWriter> {
		let (writer, reader) = track.produce();

		// NOTE: We overwrite the track if it already exists.
		self.state.lock_mut()?.tracks.insert(reader.name.clone(), reader);

		Some(writer)
	}

	pub fn remove(&mut self, track: &str) -> Option<TrackReader> {
		self.state.lock_mut()?.tracks.remove(track)
	}

	pub fn close(&mut self, code: u32) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		if let Some(mut state) = state.into_mut() {
			state.closed = Err(ServeError::Closed(code));
		}

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await
		}
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
		self.broadcast.insert(self.track.build())
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

	/// Get a track from the broadcast by name.
	///
	/// None is returned if [BroadcastWriter] cannot fufill the request.
	pub async fn subscribe(&mut self, track: Track) -> Option<TrackReader> {
		let unknown = {
			let state = self.state.lock();
			if let Some(track) = state.tracks.get(&track.name).cloned() {
				return Some(track);
			}

			state.unknown.clone()?
		};

		// TODO cache to deduplicate?
		unknown.subscribe(track).await
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await
		}
	}
}

impl Deref for BroadcastReader {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
