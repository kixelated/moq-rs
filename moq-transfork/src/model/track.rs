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
//! The track is closed with [Closed::Closed] when all writers or readers are dropped.

use super::{Group, GroupReader, GroupWriter};
pub use crate::message::GroupOrder;
use crate::{
	model::{Closed, Produce},
	runtime::Watch,
};

use std::{cmp::Ordering, ops, sync::Arc, time};

/// Static information about a track.
#[derive(Clone)]
pub struct Track {
	pub name: String,
	pub priority: u64,
	pub group_order: GroupOrder,
	pub group_expires: Option<time::Duration>,
}

impl Track {
	pub fn new<T: Into<String>>(name: T, priority: u64) -> TrackBuilder {
		TrackBuilder::new(Self {
			name: name.into(),
			priority,
			group_order: GroupOrder::Descending,
			group_expires: None,
		})
	}
}

impl Produce for Track {
	type Reader = TrackReader;
	type Writer = TrackWriter;

	fn produce(self) -> (TrackWriter, TrackReader) {
		let state = Watch::default();
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
	pub fn new(track: Track) -> Self {
		Self { track }
	}

	pub fn group_order(mut self, order: GroupOrder) -> Self {
		self.track.group_order = order;
		self
	}

	pub fn group_expires(mut self, expires: time::Duration) -> Self {
		self.track.group_expires = Some(expires);
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
	closed: Result<(), Closed>,
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
	state: Watch<TrackState>,

	// Cache the next sequence number to use
	next: u64,
}

impl TrackWriter {
	fn new(state: Watch<TrackState>, info: Arc<Track>) -> Self {
		Self { info, state, next: 0 }
	}

	// Build a new group with the given sequence number.
	pub fn create(&mut self, sequence: u64) -> Result<GroupWriter, Closed> {
		let group = Group::new(sequence);
		let (writer, reader) = group.produce();

		let mut state = self.state.lock_mut().ok_or(Closed::Cancel)?;

		if let Some(latest) = &state.latest {
			match writer.sequence.cmp(&latest.sequence) {
				Ordering::Less => return Ok(writer), // TODO dropped immediately, lul
				Ordering::Equal => return Err(Closed::Duplicate),
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

	// Build a new group with the next sequence number.
	pub fn append(&mut self) -> Result<GroupWriter, Closed> {
		self.create(self.next)
	}

	/// Close the segment with an error.
	pub fn close(&mut self, err: Closed) -> Result<(), Closed> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(Closed::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl ops::Deref for TrackWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct TrackReader {
	pub info: Arc<Track>,
	state: Watch<TrackState>,
	epoch: u64,
}

impl TrackReader {
	fn new(state: Watch<TrackState>, info: Arc<Track>) -> Self {
		Self { state, epoch: 0, info }
	}

	pub fn get(&self, sequence: u64) -> Result<GroupReader, Closed> {
		let state = self.state.lock();

		// TODO support more than just the latest group
		if let Some(latest) = &state.latest {
			if latest.sequence == sequence {
				return Ok(latest.clone());
			}
		}

		state.closed.clone()?;
		Err(Closed::Unknown)
	}

	// NOTE: This can return groups out of order.
	// TODO obey order and expires
	pub async fn next(&mut self) -> Result<Option<GroupReader>, Closed> {
		loop {
			{
				let state = self.state.lock();

				if self.epoch != state.epoch {
					self.epoch = state.epoch;
					return Ok(state.latest.clone());
				}

				state.closed.clone()?;
				match state.changed() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			}
			.await; // Try again when the state changes
		}
	}

	// Returns the largest group
	pub fn latest(&self) -> u64 {
		let state = self.state.lock();
		state.latest.as_ref().map(|group| group.sequence).unwrap_or_default()
	}

	pub async fn closed(&self) -> Result<(), Closed> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.changed() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await;
		}
	}
}

impl ops::Deref for TrackReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
