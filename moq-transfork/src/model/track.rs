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
pub use crate::message::GroupOrder;
use crate::{Error, Produce};

use std::{cmp::Ordering, fmt, ops, sync::Arc, time};

/// A track, a collection of indepedent groups (streams) with a specified order/priority.
#[derive(Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", cfg_eval::cfg_eval, serde_with::serde_as)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Track {
	/// The name of the track.
	pub name: String,

	/// The priority of the track, relative to other tracks in the same session/broadcast.
	pub priority: i8,

	/// The preferred order to deliver groups in the track.
	pub group_order: GroupOrder,

	/// The duration after which a group is considered expired.
	#[cfg_attr(feature = "serde", serde_as(as = "serde_with::DurationSecondsWithFrac"))]
	pub group_expires: time::Duration,
}

impl Track {
	pub fn new<T: Into<String>>(name: T) -> Self {
		Self::build(name).into()
	}

	pub fn build<T: Into<String>>(name: T) -> TrackBuilder {
		TrackBuilder::new(name)
	}
}

impl<T: Into<String>> From<T> for Track {
	fn from(name: T) -> Self {
		Self::new(name)
	}
}

impl fmt::Debug for Track {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.name.fmt(f)
	}
}

impl Produce for Track {
	type Consumer = TrackConsumer;
	type Producer = TrackProducer;

	fn produce(self) -> (TrackProducer, TrackConsumer) {
		let (send, recv) = watch::channel(TrackState::default());
		let info = Arc::new(self);

		let writer = TrackProducer::new(send, info.clone());
		let reader = TrackConsumer::new(recv, info);

		(writer, reader)
	}
}

/// Build a track with optional parameters.
pub struct TrackBuilder {
	track: Track,
}

impl TrackBuilder {
	pub fn new<T: Into<String>>(name: T) -> Self {
		let track = Track {
			name: name.into(),
			priority: 0,
			group_order: GroupOrder::Desc,
			group_expires: time::Duration::ZERO,
		};

		Self { track }
	}

	pub fn priority(mut self, priority: i8) -> Self {
		self.track.priority = priority;
		self
	}

	pub fn group_order(mut self, order: GroupOrder) -> Self {
		self.track.group_order = order;
		self
	}

	pub fn group_expires(mut self, expires: time::Duration) -> Self {
		self.track.group_expires = expires;
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
pub struct TrackProducer {
	pub info: Arc<Track>,
	state: watch::Sender<TrackState>,

	// Cache the next sequence number to use
	next: u64,
}

impl TrackProducer {
	fn new(state: watch::Sender<TrackState>, info: Arc<Track>) -> Self {
		Self { info, state, next: 0 }
	}

	// Build a new group with the given sequence number.
	pub fn create_group(&mut self, sequence: u64) -> GroupProducer {
		let group = Group::new(sequence);
		let (writer, reader) = group.produce();

		self.state.send_if_modified(|state| {
			if let Some(latest) = &state.latest {
				match writer.sequence.cmp(&latest.sequence) {
					Ordering::Less => return false,  // Not modified,
					Ordering::Equal => return false, // TODO error?
					Ordering::Greater => (),
				}
			}

			state.latest = Some(reader);
			self.next = sequence + 1;

			true
		});

		writer
	}

	// Build a new group with the next sequence number.
	pub fn append_group(&mut self) -> GroupProducer {
		self.create_group(self.next)
	}

	/// Close the track with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}

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
#[derive(Clone)]
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
	// TODO obey order and expires
	pub async fn next_group(&mut self) -> Result<Option<GroupConsumer>, Error> {
		// Wait until there's a new latest group or the track is closed.
		let state = match self
			.state
			.wait_for(|state| state.latest.as_ref().map(|latest| latest.sequence) != self.prev || state.closed.is_err())
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

impl fmt::Debug for TrackConsumer {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.info.name.fmt(f)
	}
}
