//! A track is a collection of semi-reliable and semi-ordered streams, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] creates streams with a sequence number and priority.
//! The sequest number is used to determine the order of streams, while the priority is used to determine which stream to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest streams may be higher priority.
//! A cloned [Publisher] can be used to create streams in parallel, but will error if a duplicate sequence number is used.
//!
//! A [Subscriber] may not receive all streams in order or at all.
//! These streams are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! streams will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Subscriber] will receive a copy of all new stream going forward (fanout).
//!
//! The track is closed with [CacheError::Closed] when all publishers or subscribers are dropped.

use crate::{error::CacheError, util::Watch};

use super::{
	datagram, Datagram, Group, GroupPublisher, GroupSubscriber, Object, ObjectHeader, ObjectPublisher, ObjectSubscriber,
};
use std::{ops::Deref, sync::Arc};

/// Static information about a track.
#[derive(Debug)]
pub struct Track {
	pub name: String,
}

// The state of the cache, depending on the mode>
enum Cache {
	Init,
	// TODO Track,
	Group(GroupSubscriber),
	Object(Vec<ObjectSubscriber>),
	Datagram(datagram::Datagram),
}

struct TrackState {
	cache: Cache,
	epoch: usize,

	// Set when the publisher is closed/dropped, or all subscribers are dropped.
	closed: Result<(), CacheError>,
}

impl TrackState {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}

	pub fn insert_group(&mut self, group: GroupSubscriber) -> Result<(), CacheError> {
		self.closed.clone()?;

		match &self.cache {
			Cache::Init => {}
			Cache::Group(old) => {
				if old.id == group.id {
					return Err(CacheError::Duplicate);
				} else if old.id > group.id {
					return Ok(());
				}
			}
			_ => return Err(CacheError::Mode),
		};

		self.cache = Cache::Group(group);
		self.epoch += 1;

		Ok(())
	}

	pub fn insert_object(&mut self, object: ObjectSubscriber) -> Result<(), CacheError> {
		self.closed.clone()?;

		match &mut self.cache {
			Cache::Init => {
				self.cache = Cache::Object(vec![object]);
			}
			Cache::Object(objects) => {
				let first = objects.first().unwrap();

				if first.group_id > object.group_id {
					// Drop this old group
					return Ok(());
				} else if first.group_id < object.group_id {
					objects.clear()
				}

				objects.push(object);
			}
			_ => return Err(CacheError::Mode),
		};

		self.epoch += 1;

		Ok(())
	}

	pub fn insert_datagram(&mut self, datagram: datagram::Datagram) -> Result<(), CacheError> {
		self.closed.clone()?;

		match &self.cache {
			Cache::Init | Cache::Datagram(_) => {}
			_ => return Err(CacheError::Mode),
		};

		self.cache = Cache::Datagram(datagram);
		self.epoch += 1;

		Ok(())
	}
}

impl Default for TrackState {
	fn default() -> Self {
		Self {
			cache: Cache::Init,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

/// Creates new streams for a track.
pub struct TrackPublisher {
	state: Watch<TrackState>,
	info: Arc<Track>,
	subscriber: TrackSubscriber,
}

impl TrackPublisher {
	/// Create a track with the given name.
	pub fn new(info: Track) -> Self {
		let state = Watch::new(TrackState::default());
		let info = Arc::new(info);

		let subscriber = TrackSubscriber::new(state.clone(), info.clone());

		Self {
			state,
			info,
			subscriber,
		}
	}

	// TODO support entire track as an stream

	/// Create a group with the given info.
	pub fn create_group(&mut self, info: Group) -> Result<GroupPublisher, CacheError> {
		let publisher = GroupPublisher::new(info);
		self.state.lock_mut().insert_group(publisher.subscribe())?;
		Ok(publisher)
	}

	/// Create an object with the given info and payload.
	pub fn create_object(&mut self, full: Object) -> Result<(), CacheError> {
		let payload = full.payload.clone();
		let mut publisher = ObjectPublisher::new(full.into());
		publisher.chunk(payload)?;
		self.state.lock_mut().insert_object(publisher.subscribe())?;
		Ok(())
	}

	/// Create an object with the given info and size, but no payload yet.
	pub fn create_object_chunked(&mut self, info: ObjectHeader) -> Result<ObjectPublisher, CacheError> {
		let publisher = ObjectPublisher::new(info);
		self.state.lock_mut().insert_object(publisher.subscribe())?;
		Ok(publisher)
	}

	/// Create a datagram that is not cached.
	pub fn create_datagram(&mut self, info: datagram::Datagram) -> Result<(), CacheError> {
		self.state.lock_mut().insert_datagram(info)?;
		Ok(())
	}

	pub fn subscribe(&self) -> TrackSubscriber {
		self.subscriber.clone()
	}

	/// Close the stream with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for TrackPublisher {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Receives new streams for a track.
#[derive(Clone)]
pub struct TrackSubscriber {
	state: Watch<TrackState>,
	info: Arc<Track>,
	epoch: usize,
}

impl TrackSubscriber {
	fn new(state: Watch<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info, epoch: 0 }
	}

	/// Block until the next stream arrives
	pub async fn next(&mut self) -> Result<TrackMode, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.epoch != state.epoch {
					match &state.cache {
						Cache::Init => {}
						Cache::Group(group) => {
							self.epoch = state.epoch;
							return Ok(group.clone().into());
						}
						Cache::Object(objects) => {
							let index = objects.len().saturating_sub(state.epoch - self.epoch);
							self.epoch = state.epoch - objects.len() + index + 1;
							return Ok(objects[index].clone().into());
						}
						Cache::Datagram(datagram) => {
							self.epoch = state.epoch;
							return Ok(datagram.clone().into());
						}
					}
				}

				// Otherwise check if we need to return an error.
				state.closed.clone()?;
				state.changed()
			};

			notify.await
		}
	}
}

impl Deref for TrackSubscriber {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub enum TrackMode {
	// TODO Track
	Group(GroupSubscriber),
	Object(ObjectSubscriber),
	Datagram(Datagram),
}

impl From<GroupSubscriber> for TrackMode {
	fn from(subscriber: GroupSubscriber) -> Self {
		Self::Group(subscriber)
	}
}

impl From<ObjectSubscriber> for TrackMode {
	fn from(subscriber: ObjectSubscriber) -> Self {
		Self::Object(subscriber)
	}
}

impl From<datagram::Datagram> for TrackMode {
	fn from(info: Datagram) -> Self {
		Self::Datagram(info)
	}
}
