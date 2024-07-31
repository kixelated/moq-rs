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
use std::{collections::HashMap, ops, sync::Arc};

use super::{Produce, RouterReader, Track, TrackBuilder, TrackReader, TrackWriter};
use crate::{runtime::Watch, MoqError};

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

impl Produce for Broadcast {
	type Reader = BroadcastReader;
	type Writer = BroadcastWriter;

	fn produce(self) -> (BroadcastWriter, BroadcastReader) {
		let info = Arc::new(self);
		let state = Watch::default();

		let writer = BroadcastWriter::new(state.split(), info.clone());
		let reader = BroadcastReader::new(state, info);

		(writer, reader)
	}
}

pub struct BroadcastState {
	tracks: HashMap<String, TrackReader>,
	router: Option<RouterReader<Track>>,
	closed: Result<(), MoqError>,
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
pub struct BroadcastWriter {
	state: Watch<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastWriter {
	fn new(state: Watch<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	pub fn create_track<T: Into<String>>(&mut self, name: T, priority: u64) -> BroadcastTrackBuilder {
		BroadcastTrackBuilder::new(self, name.into(), priority)
	}

	/// Optionally route requests for unknown tracks.
	pub fn route_tracks(&mut self, router: RouterReader<Track>) -> Result<(), MoqError> {
		self.state.lock_mut().ok_or(MoqError::Cancel)?.router = Some(router);
		Ok(())
	}

	/// Insert a track into the broadcast.
	pub fn insert_track(&mut self, track: Track) -> Result<TrackWriter, MoqError> {
		let (writer, reader) = track.produce();

		// NOTE: We overwrite the track if it already exists.
		self.state
			.lock_mut()
			.ok_or(MoqError::Cancel)?
			.tracks
			.insert(reader.name.clone(), reader);

		Ok(writer)
	}

	pub fn remove_track(&mut self, track: &str) -> Option<TrackReader> {
		self.state.lock_mut()?.tracks.remove(track)
	}

	pub fn close(&mut self, code: u32) -> Result<(), MoqError> {
		let state = self.state.lock();
		state.closed.clone()?;
		state.into_mut().ok_or(MoqError::Cancel)?.closed = Err(MoqError::App(code));

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), MoqError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.changed() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await
		}
	}
}

impl ops::Deref for BroadcastWriter {
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
	fn new(broadcast: &'a mut BroadcastWriter, name: String, priority: u64) -> Self {
		Self {
			track: Track::new(name, priority),
			broadcast,
		}
	}

	pub fn build(self) -> Result<TrackWriter, MoqError> {
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
	state: Watch<BroadcastState>,
	pub info: Arc<Broadcast>,
}

impl BroadcastReader {
	fn new(state: Watch<BroadcastState>, info: Arc<Broadcast>) -> Self {
		Self { state, info }
	}

	/// Get a track from the broadcast by name.
	pub async fn subscribe(&self, track: Track) -> Result<TrackReader, MoqError> {
		let router = {
			let state = self.state.lock();
			if let Some(track) = state.tracks.get(&track.name).cloned() {
				return Ok(track);
			}

			state.router.clone().ok_or(MoqError::NotFound)?
		};

		// TODO cache to deduplicate?
		router.subscribe(track).await
	}

	pub async fn closed(&self) -> Result<(), MoqError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.changed() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await
		}
	}
}

impl ops::Deref for BroadcastReader {
	type Target = Broadcast;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
