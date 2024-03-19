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

use crate::{error::CacheError, publisher, util::Watch, ServeError};

use super::{
	Datagram, Group, GroupPublisher, GroupSubscriber, Object, ObjectHeader, ObjectPublisher, ObjectSubscriber, Stream,
	StreamPublisher, StreamSubscriber,
};
use futures::stream::FuturesUnordered;
use futures::stream::StreamExt;
use futures::FutureExt;
use std::{ops::Deref, sync::Arc};

/// Static information about a track.
#[derive(Debug)]
pub struct Track {
	pub namespace: String,
	pub name: String,
}

impl Track {
	pub fn produce(self) -> (TrackPublisher, TrackSubscriber) {
		let state = Watch::new(State::default());
		let info = Arc::new(self);

		let publisher = TrackPublisher::new(state.clone(), info.clone());
		let subscriber = TrackSubscriber::new(state, info);

		(publisher, subscriber)
	}
}

// The state of the cache, depending on the mode>
enum Cache {
	Init,
	Stream(StreamSubscriber),
	Group(GroupSubscriber),
	Object(Vec<ObjectSubscriber>),
	Datagram(Datagram),
}

struct State {
	cache: Cache,
	epoch: usize,

	// Set when the publisher is closed/dropped, or all subscribers are dropped.
	closed: Result<(), CacheError>,
}

impl State {
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

	pub fn insert_datagram(&mut self, datagram: Datagram) -> Result<(), CacheError> {
		self.closed.clone()?;

		match &self.cache {
			Cache::Init | Cache::Datagram(_) => {}
			_ => return Err(CacheError::Mode),
		};

		self.cache = Cache::Datagram(datagram);
		self.epoch += 1;

		Ok(())
	}

	pub fn set_stream(&mut self, stream: StreamSubscriber) -> Result<(), CacheError> {
		self.closed.clone()?;

		match &self.cache {
			Cache::Init => {}
			_ => return Err(CacheError::Mode),
		};

		self.cache = Cache::Stream(stream);
		self.epoch += 1;

		Ok(())
	}
}

impl Default for State {
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
	state: Watch<State>,
	info: Arc<Track>,
}

impl TrackPublisher {
	/// Create a track with the given name.
	fn new(state: Watch<State>, info: Arc<Track>) -> Self {
		Self { state, info }
	}

	/// Create a group with the given info.
	pub fn create_group(&mut self, group: Group) -> Result<GroupPublisher, CacheError> {
		let (publisher, subscriber) = group.produce();
		self.state.lock_mut().insert_group(subscriber)?;
		Ok(publisher)
	}

	/// Create an object with the given info and payload.
	pub fn write_object(&mut self, object: Object) -> Result<(), CacheError> {
		let payload = object.payload.clone();
		let header = ObjectHeader::from(object);
		let (mut publisher, subscriber) = header.produce();
		publisher.write(payload)?;
		self.state.lock_mut().insert_object(subscriber)?;
		Ok(())
	}

	/// Create an object with the given info and size, but no payload yet.
	pub fn create_object(&mut self, object: ObjectHeader) -> Result<ObjectPublisher, CacheError> {
		let (publisher, subscriber) = object.produce();
		self.state.lock_mut().insert_object(subscriber)?;
		Ok(publisher)
	}

	/// Create a datagram that is not cached.
	pub fn create_datagram(&mut self, info: Datagram) -> Result<(), CacheError> {
		self.state.lock_mut().insert_datagram(info)?;
		Ok(())
	}

	/// Create a single stream for the entire track, served in strict order.
	pub fn create_stream(&mut self, send_order: u64) -> Result<StreamPublisher, CacheError> {
		let (publisher, subscriber) = Stream {
			namespace: self.namespace.clone(),
			name: self.name.clone(),
			send_order,
		}
		.produce();
		self.state.lock_mut().set_stream(subscriber)?;
		Ok(publisher)
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
	state: Watch<State>,
	info: Arc<Track>,
	epoch: usize,
	_dropped: Arc<Dropped>,
}

impl TrackSubscriber {
	fn new(state: Watch<State>, info: Arc<Track>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			epoch: 0,
			_dropped,
		}
	}

	/// Block until the next stream arrives
	pub async fn next(&mut self) -> Result<TrackMode, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.epoch != state.epoch {
					match &state.cache {
						Cache::Init => {}
						Cache::Stream(stream) => {
							self.epoch = state.epoch;
							return Ok(stream.clone().into());
						}
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

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		let state = self.state.lock();
		match &state.cache {
			Cache::Init => None,
			Cache::Datagram(datagram) => Some((datagram.group_id, datagram.object_id)),
			Cache::Group(group) => Some((group.id, group.latest())),
			Cache::Object(objects) => objects
				.iter()
				.max_by_key(|a| (a.group_id, a.object_id))
				.map(|a| (a.group_id, a.object_id)),
			Cache::Stream(stream) => stream.latest(),
		}
	}

	pub async fn serve(mut self, dst: publisher::Subscribe) -> Result<(), ServeError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				next = self.next() =>
					match next? {
						TrackMode::Stream(stream) => return stream.serve(dst).await,
						TrackMode::Group(group) => tasks.push(group.serve(dst.clone()).boxed()),
						TrackMode::Object(object) => tasks.push(object.serve(dst.clone()).boxed()),
						TrackMode::Datagram(datagram) => datagram.serve(dst.clone())?,
					},
				res = tasks.next(), if !tasks.is_empty() => res.unwrap()?,
			}
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
	Stream(StreamSubscriber),
	Group(GroupSubscriber),
	Object(ObjectSubscriber),
	Datagram(Datagram),
}

impl From<StreamSubscriber> for TrackMode {
	fn from(subscriber: StreamSubscriber) -> Self {
		Self::Stream(subscriber)
	}
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

impl From<Datagram> for TrackMode {
	fn from(info: Datagram) -> Self {
		Self::Datagram(info)
	}
}

struct Dropped {
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(CacheError::Done).ok();
	}
}
