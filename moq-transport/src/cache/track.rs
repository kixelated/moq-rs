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

use super::{datagram, group, object};
use std::{fmt, ops::Deref, sync::Arc};

/// Create a track with the given name.
pub fn new(name: &str) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(Info { name: name.to_string() });

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about a track.
#[derive(Debug)]
pub struct Info {
	pub name: String,
}

// The state of the cache, depending on the mode>
#[derive(Debug)]
enum Cache {
	Init,
	// TODO Track,
	Group(group::Subscriber),
	Object(Vec<object::Subscriber>),
	Datagram(datagram::Info),
}

#[derive(Debug)]
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

	pub fn insert_group(&mut self, group: group::Subscriber) -> Result<(), CacheError> {
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

	pub fn insert_object(&mut self, object: object::Subscriber) -> Result<(), CacheError> {
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

	pub fn insert_datagram(&mut self, datagram: datagram::Info) -> Result<(), CacheError> {
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
pub struct Publisher {
	state: Watch<State>,
	info: Arc<Info>,
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	// TODO support entire track as an stream

	/// Create a group with the given info.
	pub fn create_group(&mut self, info: group::Info) -> Result<group::Publisher, CacheError> {
		let (publisher, subscriber) = group::new(info);
		self.state.lock_mut().insert_group(subscriber)?;
		Ok(publisher)
	}

	/// Create an object with the given info and payload.
	pub fn create_object(&mut self, full: object::Full) -> Result<(), CacheError> {
		let payload = full.payload.clone();
		let (mut publisher, subscriber) = object::new(full.into());
		publisher.chunk(payload)?;
		self.state.lock_mut().insert_object(subscriber)?;
		Ok(())
	}

	/// Create an object with the given info and size, but no payload yet.
	pub fn create_object_chunked(&mut self, info: object::Info) -> Result<object::Publisher, CacheError> {
		let (publisher, subscriber) = object::new(info);
		self.state.lock_mut().insert_object(subscriber)?;
		Ok(publisher)
	}

	/// Create a datagram that is not cached.
	pub fn create_datagram(&mut self, info: datagram::Info) -> Result<(), CacheError> {
		self.state.lock_mut().insert_datagram(info)?;
		Ok(())
	}

	/// Close the stream with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Publisher")
			.field("state", &self.state)
			.field("info", &self.info)
			.finish()
	}
}

/// Receives new streams for a track.
#[derive(Clone)]
pub struct Subscriber {
	state: Watch<State>,
	info: Arc<Info>,
	epoch: usize,

	// Dropped when all subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			epoch: 0,
			_dropped,
		}
	}

	/// Block until the next stream arrives
	pub async fn next(&mut self) -> Result<Mode, CacheError> {
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

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Subscriber")
			.field("state", &self.state)
			.field("info", &self.info)
			.field("epoch", &self.epoch)
			.finish()
	}
}

// Closes the track on Drop.
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

pub enum Mode {
	// TODO Track
	Group(group::Subscriber),
	Object(object::Subscriber),
	Datagram(datagram::Info),
}

impl From<group::Subscriber> for Mode {
	fn from(subscriber: group::Subscriber) -> Self {
		Self::Group(subscriber)
	}
}

impl From<object::Subscriber> for Mode {
	fn from(subscriber: object::Subscriber) -> Self {
		Self::Object(subscriber)
	}
}

impl From<datagram::Info> for Mode {
	fn from(info: datagram::Info) -> Self {
		Self::Datagram(info)
	}
}
