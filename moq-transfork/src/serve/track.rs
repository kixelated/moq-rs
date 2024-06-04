//! A track is a collection of semi-reliable and semi-ordered streams, split into a [Writer] and [Reader] handle.
//!
//! A [Writer] creates streams with a sequence number and priority.
//! The sequest number is used to determine the order of streams, while the priority is used to determine which stream to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest streams may be higher priority.
//! A cloned [Writer] can be used to create streams in parallel, but will error if a duplicate sequence number is used.
//!
//! A [Reader] may not receive all streams in order or at all.
//! These streams are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! streams will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Reader] will receive a copy of all new stream going forward (fanout).
//!
//! The track is closed with [ServeError::Closed] when all writers or readers are dropped.

use crate::util::State;

use super::{Group, GroupReader, GroupWriter, ServeError};
use std::{cmp::Ordering, ops::Deref, sync::Arc};

/// Static information about a track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
	pub name: String,
	pub order: Option<TrackOrder>,
	pub priority: Option<u64>,
}

impl Track {
	pub fn new(name: &str) -> TrackBuilder {
		TrackBuilder::new(Self {
			name: name.to_string(),
			order: None,
			priority: None,
		})
	}

	pub fn produce(self) -> (TrackWriter, TrackReader) {
		let state = State::default();
		let info = Arc::new(self);

		let writer = TrackWriter::new(state.split(), info.clone());
		let reader = TrackReader::new(state, info);

		(writer, reader)
	}
}

pub struct TrackBuilder {
	track: Track,
}

impl TrackBuilder {
	fn new(track: Track) -> Self {
		Self { track }
	}

	pub fn order(mut self, order: TrackOrder) -> Self {
		self.track.order = Some(order);
		self
	}

	pub fn priority(mut self, priority: u64) -> Self {
		self.track.priority = Some(priority);
		self
	}

	pub fn build(self) -> Track {
		self.track
	}

	pub fn produce(self) -> (TrackWriter, TrackReader) {
		self.build().produce()
	}
}

struct TrackState {
	latest: Option<GroupReader>,
	epoch: u64, // Updated each time latest changes
	closed: Result<(), ServeError>,
}

impl Default for TrackState {
	fn default() -> Self {
		Self {
			latest: None,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

pub struct TrackWriter {
	pub info: Arc<Track>,
	state: State<TrackState>,

	// Cache the next sequence number to use
	next: u64,
}

impl TrackWriter {
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self { info, state, next: 0 }
	}

	pub fn create_group(&mut self, group: Group) -> Result<GroupWriter, ServeError> {
		let (writer, reader) = group.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;

		if let Some(latest) = &state.latest {
			match writer.sequence.cmp(&latest.sequence) {
				Ordering::Less => return Ok(writer), // TODO dropped immediately, lul
				Ordering::Equal => return Err(ServeError::Duplicate),
				Ordering::Greater => state.latest = Some(reader),
			}
		} else {
			state.latest = Some(reader);
		}

		state.epoch += 1;

		// Cache the next sequence number
		self.next = state.latest.as_ref().unwrap().sequence + 1;

		Ok(writer)
	}

	// Helper to replace the sequence number with the next one.
	pub fn append_group(&mut self, mut group: Group) -> Result<GroupWriter, ServeError> {
		group.sequence = self.next_group();
		self.create_group(group)
	}

	// Helper to return the sequence number for the next group.
	// TODO avoid this unnecessary mutex
	pub fn next_group(&self) -> u64 {
		self.next
	}

	/// Close the segment with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Deref for TrackWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct TrackReader {
	pub info: Arc<Track>,
	state: State<TrackState>,
	epoch: u64,

	pub priority: Option<u64>,
	pub order: Option<TrackOrder>,
}

impl TrackReader {
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self {
			state,
			epoch: 0,
			order: info.order,
			priority: info.priority,
			info,
		}
	}

	pub fn get_group(&self, sequence: u64) -> Option<GroupReader> {
		let state = self.state.lock();

		// TODO support more than just the latest group
		state
			.latest
			.as_ref()
			.filter(|group| group.sequence == sequence)
			.cloned()
	}

	// NOTE: This can return groups out of order.
	// TODO obey order and expires
	pub async fn next_group(&mut self) -> Result<Option<GroupReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();

				if self.epoch != state.epoch {
					self.epoch = state.epoch;
					return Ok(state.latest.clone());
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			}
			.await; // Try again when the state changes
		}
	}

	// Returns the largest group
	pub fn latest(&self) -> Option<u64> {
		let state = self.state.lock();
		state.latest.as_ref().map(|group| group.sequence)
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
			.await;
		}
	}
}

impl Deref for TrackReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrackOrder {
	Ascending,
	Descending,
}
