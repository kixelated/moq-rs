//! A track is a collection of semi-reliable and semi-ordered streams, split into a [Producer] and [Consumer] handle.
//!
//! A [Producer] creates streams with a sequence number and priority.
//! The sequest number is used to determine the order of streams, while the priority is used to determine which stream to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest streams may be higher priority.
//! A cloned [Producer] can be used to create streams in parallel, but will error if a duplicate sequence number is used.
//!
//! A [Consumer] may not receive all streams in order or at all.
//! These streams are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! streams will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Consumer] will receive a copy of all new stream going forward (fanout).
//!
//! The track is closed with [Error::Error] when all writers or readers are dropped.

use tokio::sync::watch;

use super::{Group, GroupConsumer, GroupProducer};
pub use crate::message::GroupOrder;
use crate::{Error, Produce};

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
	pub fn build<T: Into<String>>(name: T, priority: u64) -> TrackBuilder {
		TrackBuilder::new(Self {
			name: name.into(),
			priority,
			group_order: GroupOrder::Descending,
			group_expires: None,
		})
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
	epoch: u64, // +1 each change
}

impl Default for TrackState {
	fn default() -> Self {
		Self {
			latest: None,
			closed: Ok(()),
			epoch: 0,
		}
	}
}

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
			state.epoch += 1;

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

#[derive(Clone)]
pub struct TrackConsumer {
	pub info: Arc<Track>,
	state: watch::Receiver<TrackState>,
	epoch: u64,
}

impl TrackConsumer {
	fn new(state: watch::Receiver<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info, epoch: 0 }
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
		loop {
			{
				let state = self.state.borrow_and_update();
				if let Some(latest) = state.latest.as_ref() {
					if self.epoch != latest.sequence {
						self.epoch = latest.sequence;
						return Ok(Some(latest.clone()));
					}
				}

				state.closed.clone()?;
			}

			if self.state.changed().await.is_err() {
				return Ok(None);
			}
		}
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
