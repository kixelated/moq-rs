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

use super::{Group, GroupConsumer, GroupProducer, Path};
pub use crate::message::GroupOrder;
use crate::Error;

use std::{cmp::Ordering, ops, sync::Arc};

/// A track, a collection of indepedent groups (streams) with a specified order/priority.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Track {
	/// The path of the track.
	pub path: Path,

	/// The priority of the track, relative to other tracks in the same session/broadcast.
	pub priority: i8,

	/// The preferred order to deliver groups in the track.
	pub order: GroupOrder,
}

impl Track {
	pub fn new(path: Path) -> Self {
		Self {
			path,
			..Default::default()
		}
	}

	pub fn build() -> TrackBuilder {
		TrackBuilder::new()
	}

	pub fn produce(self) -> (TrackProducer, TrackConsumer) {
		let (send, recv) = watch::channel(TrackState::default());
		let info = Arc::new(self);

		let writer = TrackProducer::new(send, info.clone());
		let reader = TrackConsumer::new(recv, info);

		(writer, reader)
	}
}

impl Default for Track {
	fn default() -> Self {
		Self {
			path: Default::default(),
			priority: 0,
			order: GroupOrder::Desc,
		}
	}
}

/// Build a track with optional parameters.
pub struct TrackBuilder {
	track: Track,
}

impl Default for TrackBuilder {
	fn default() -> Self {
		Self::new()
	}
}

impl TrackBuilder {
	pub fn new() -> Self {
		Self {
			track: Default::default(),
		}
	}

	pub fn path<T: ToString>(mut self, part: T) -> Self {
		self.track.path = self.track.path.push(part);
		self
	}

	pub fn priority(mut self, priority: i8) -> Self {
		self.track.priority = priority;
		self
	}

	pub fn group_order(mut self, order: GroupOrder) -> Self {
		self.track.order = order;
		self
	}

	pub fn produce(self) -> (TrackProducer, TrackConsumer) {
		self.track.produce()
	}

	// I don't know why From isn't sufficient, but this prevents annoying Rust errors.
	pub fn into(self) -> Track {
		self.track
	}
}

impl From<TrackBuilder> for Track {
	fn from(builder: TrackBuilder) -> Self {
		builder.track
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
	pub info: Arc<Track>,
	state: watch::Sender<TrackState>,
}

impl TrackProducer {
	fn new(state: watch::Sender<TrackState>, info: Arc<Track>) -> Self {
		Self { info, state }
	}

	/// Build a new group with the given sequence number.
	pub fn create_group(&mut self, sequence: u64) -> GroupProducer {
		let group = Group::new(sequence);
		let (writer, reader) = group.produce();

		self.state.send_if_modified(|state| {
			if let Some(latest) = &state.latest {
				match reader.sequence.cmp(&latest.sequence) {
					Ordering::Less => return false,  // Not modified,
					Ordering::Equal => return false, // TODO error?
					Ordering::Greater => (),
				}
			}

			state.latest = Some(reader);
			true
		});

		writer
	}

	/// Build a new group with the next sequence number.
	pub fn append_group(&mut self) -> GroupProducer {
		// TODO remove this extra lock
		let sequence = self
			.state
			.borrow()
			.latest
			.as_ref()
			.map_or(0, |group| group.sequence + 1);

		self.create_group(sequence)
	}

	/// Close the track with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}

	/// Create a new consumer for the track.
	pub fn subscribe(&self) -> TrackConsumer {
		TrackConsumer::new(self.state.subscribe(), self.info.clone())
	}

	/// Block until there are no active consumers.
	pub async fn unused(&self) {
		self.state.closed().await
	}
}

impl ops::Deref for TrackProducer {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// A consumer for a track, used to read groups.
#[derive(Clone, Debug)]
pub struct TrackConsumer {
	pub info: Arc<Track>,
	state: watch::Receiver<TrackState>,
	prev: Option<u64>, // The previous sequence number
}

impl TrackConsumer {
	fn new(state: watch::Receiver<TrackState>, info: Arc<Track>) -> Self {
		Self {
			state,
			info,
			prev: None,
		}
	}

	pub fn get_group(&self, sequence: u64) -> Result<GroupConsumer, Error> {
		let state = self.state.borrow();

		// TODO support more than just the latest group
		if let Some(latest) = &state.latest {
			if latest.sequence == sequence {
				return Ok(latest.clone());
			}
		}

		state.closed.clone()?;
		Err(Error::NotFound)
	}

	// NOTE: This can return groups out of order.
	// TODO obey order
	pub async fn next_group(&mut self) -> Result<Option<GroupConsumer>, Error> {
		// Wait until there's a new latest group or the track is closed.
		let state = match self
			.state
			.wait_for(|state| state.latest.as_ref().map(|group| group.sequence) != self.prev || state.closed.is_err())
			.await
		{
			Ok(state) => state,
			Err(_) => return Ok(None),
		};

		// If there's a new latest group, return it.
		if let Some(group) = state.latest.as_ref() {
			if Some(group.sequence) != self.prev {
				self.prev = Some(group.sequence);
				return Ok(Some(group.clone()));
			}
		}

		// Otherwise the track is closed.
		Err(state.closed.clone().unwrap_err())
	}

	// Returns the largest group
	pub fn latest_group(&self) -> u64 {
		let state = self.state.borrow();
		state.latest.as_ref().map(|group| group.sequence).unwrap_or_default()
	}

	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}

impl ops::Deref for TrackConsumer {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
