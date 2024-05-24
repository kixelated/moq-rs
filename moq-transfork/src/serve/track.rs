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

use crate::watch::State;

use super::{Group, GroupInfo, GroupReader, GroupWriter, ServeError};
use std::{cmp::Ordering, ops::Deref, sync::Arc};

/// Static information about a track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
	pub namespace: String,
	pub name: String,
}

impl Track {
	pub fn new(namespace: String, name: String) -> Self {
		Self { namespace, name }
	}

	pub fn produce(self) -> (TrackWriter, TrackReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = TrackWriter::new(writer, info.clone());
		let reader = TrackReader::new(reader, info);

		(writer, reader)
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
	next: u64, // Not in the state to avoid a lock
}

impl TrackWriter {
	fn new(state: State<TrackState>, track: Arc<Track>) -> Self {
		Self {
			info: track,
			state,
			next: 0,
		}
	}

	// Helper to increment the group by one.
	pub fn append(&mut self, priority: u64) -> Result<GroupWriter, ServeError> {
		self.create(Group {
			group_id: self.next,
			priority,
		})
	}

	pub fn create(&mut self, group: Group) -> Result<GroupWriter, ServeError> {
		let group = GroupInfo {
			track: self.info.clone(),
			group_id: group.group_id,
			priority: group.priority,
		};
		let (writer, reader) = group.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;

		if let Some(latest) = &state.latest {
			match writer.group_id.cmp(&latest.group_id) {
				Ordering::Less => return Ok(writer), // dropped immediately, lul
				Ordering::Equal => return Err(ServeError::Duplicate),
				Ordering::Greater => state.latest = Some(reader),
			}
		} else {
			state.latest = Some(reader);
		}

		self.next = state.latest.as_ref().unwrap().group_id + 1;
		state.epoch += 1;

		Ok(writer)
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
}

impl TrackReader {
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self { info, state, epoch: 0 }
	}

	pub async fn next(&mut self) -> Result<Option<GroupReader>, ServeError> {
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

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		let state = self.state.lock();
		state.latest.as_ref().map(|group| (group.group_id, group.latest()))
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
