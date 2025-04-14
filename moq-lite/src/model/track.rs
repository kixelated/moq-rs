//! A track is a collection of semi-reliable and semi-ordered streams, split into a [TrackProducer] and [TrackConsumer] handle.
//!
//! A [TrackProducer] creates streams with a sequence number and priority.
//! The sequest number is used to determine the order of streams, while the priority is used to determine which stream to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest streams may be higher priority.
//! A cloned [Producer] can be used to create streams in parallel, but will error if a duplicate sequence number is used.
//!
//! A [TrackConsumer] may not receive all streams in order or at all.
//! These streams are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! streams will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Consumer] will receive a copy of all new stream going forward (fanout).
//!
//! The track is closed with [Error] when all writers or readers are dropped.

use tokio::sync::watch;

use super::{Group, GroupConsumer, GroupProducer};
use crate::Error;

use std::cmp::Ordering;

#[derive(Clone, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Track {
	pub name: String,
	pub priority: i8,
}

impl Track {
	pub fn new(name: String, priority: i8) -> Self {
		Self { name, priority }
	}

	pub fn produce(self) -> TrackProducer {
		TrackProducer::new(self)
	}
}

#[derive(Debug)]
struct TrackState {
	latest: Option<GroupConsumer>,
	closed: Result<(), Error>,
}

impl Default for TrackState {
	fn default() -> Self {
		Self {
			latest: None,
			closed: Ok(()),
		}
	}
}

/// A producer for a track, used to create new groups.
#[derive(Clone, Debug)]
pub struct TrackProducer {
	pub info: Track,
	state: watch::Sender<TrackState>,
}

impl TrackProducer {
	pub fn new(info: Track) -> Self {
		Self {
			info,
			state: Default::default(),
		}
	}

	/// Insert a group into the track, returning true if this is the latest group.
	pub fn insert_group(&mut self, group: GroupConsumer) -> bool {
		self.state.send_if_modified(|state| {
			if let Some(latest) = &state.latest {
				match group.info.cmp(&latest.info) {
					Ordering::Less => return false,
					Ordering::Equal => return false,
					Ordering::Greater => (),
				}
			}

			state.latest = Some(group.clone());
			true
		})
	}

	/// Create a new group with the given sequence number.
	///
	/// If the sequence number is not the latest, this method will return None.
	pub fn create_group(&mut self, info: Group) -> Option<GroupProducer> {
		let group = GroupProducer::new(info);
		self.insert_group(group.consume()).then_some(group)
	}

	/// Create a new group with the next sequence number.
	pub fn append_group(&mut self) -> GroupProducer {
		// TODO remove this extra lock
		let sequence = self
			.state
			.borrow()
			.latest
			.as_ref()
			.map_or(0, |group| group.info.sequence + 1);

		let group = Group { sequence };
		self.create_group(group).unwrap()
	}

	/// Close the track with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}

	/// Create a new consumer for the track.
	pub fn consume(&self) -> TrackConsumer {
		TrackConsumer {
			info: self.info.clone(),
			state: self.state.subscribe(),
			prev: None,
		}
	}

	/// Block until there are no active consumers.
	pub async fn unused(&self) {
		self.state.closed().await
	}
}

impl From<Track> for TrackProducer {
	fn from(info: Track) -> Self {
		TrackProducer::new(info)
	}
}

/// A consumer for a track, used to read groups.
#[derive(Clone, Debug)]
pub struct TrackConsumer {
	pub info: Track,
	state: watch::Receiver<TrackState>,
	prev: Option<u64>, // The previous sequence number
}

impl TrackConsumer {
	/// Return the next group in order.
	///
	/// NOTE: This can have gaps if the reader is too slow or there were network slowdowns.
	pub async fn next_group(&mut self) -> Result<Option<GroupConsumer>, Error> {
		// Wait until there's a new latest group or the track is closed.
		let state = match self
			.state
			.wait_for(|state| {
				state.latest.as_ref().map(|group| group.info.sequence) != self.prev || state.closed.is_err()
			})
			.await
		{
			Ok(state) => state,
			Err(_) => return Ok(None),
		};

		// If there's a new latest group, return it.
		if let Some(group) = state.latest.as_ref() {
			if Some(group.info.sequence) != self.prev {
				self.prev = Some(group.info.sequence);
				return Ok(Some(group.clone()));
			}
		}

		// Otherwise the track is closed.
		Err(state.closed.clone().unwrap_err())
	}

	/// Block until the track is closed and return the error.
	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}
