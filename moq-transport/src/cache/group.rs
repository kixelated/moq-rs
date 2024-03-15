//! A stream is a stream of objects with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of objects.
//! Each object can have a sequence number, allowing the subscriber to detect gaps objects.
//!
//! A [Subscriber] reads an ordered stream of objects.
//! The subscriber can be cloned, in which case each subscriber receives a copy of each object. (fanout)
//!
//! The stream is closed with [CacheError::Closed] when all publishers or subscribers are dropped.
use core::fmt;
use std::{ops::Deref, sync::Arc};

use crate::{error::CacheError, util::Watch};

use super::object;

/// Create a new stream with the given info.
pub fn new(info: Info) -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());
	let info = Arc::new(info);

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

/// Static information about the stream.
#[derive(Debug)]
pub struct Info {
	// The sequence number of the stream within the track.
	// NOTE: These may be received out of order or with gaps.
	pub id: u64,

	// The priority of the stream within the BROADCAST.
	pub send_order: u64,
}

struct State {
	// The data that has been received thus far.
	objects: Vec<object::Subscriber>,

	// Set when the publisher is dropped.
	closed: Result<(), CacheError>,
}

impl State {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			closed: Ok(()),
		}
	}
}

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("State")
			.field("objects", &self.objects)
			.field("closed", &self.closed)
			.finish()
	}
}

/// Used to write data to a stream and notify subscribers.
pub struct Publisher {
	// Mutable stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Info>,

	// The next object sequence number to use.
	next: u64,

	// Closes the stream when all Publishers are dropped.
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			next: 0,
			_dropped,
		}
	}

	/// Write an object with the given payload.
	pub fn create_object(&mut self, payload: bytes::Bytes) -> Result<(), CacheError> {
		let mut object = self.create_object_chunked(payload.len())?;
		object.chunk(payload)?;
		Ok(())
	}

	/// Write an object over multiple writes.
	///
	/// BAD STUFF will happen if the size is wrong.
	pub fn create_object_chunked(&mut self, size: usize) -> Result<object::Publisher, CacheError> {
		let (publisher, subscriber) = object::new(object::Info {
			group_id: self.info.id,
			object_id: self.next.try_into().unwrap(),
			send_order: self.info.send_order,
			size,
		});
		self.next += 1;

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.objects.push(subscriber);
		Ok(publisher)
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

/// Notified when a stream has new data available.
#[derive(Clone)]
pub struct Subscriber {
	// Modify the stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Info>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	// Dropped when all Subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));

		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	/// Block until the next object is available.
	pub async fn object(&mut self) -> Result<object::Subscriber, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.index < state.objects.len() {
					let object = state.objects[self.index].clone();
					self.index += 1;
					return Ok(object);
				}

				state.closed.clone()?;
				state.changed()
			};

			notify.await; // Try again when the state changes
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
			.field("index", &self.index)
			.finish()
	}
}

struct Dropped {
	// Modify the stream state.
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

/// A subset of Object::Info, since we use the group's info.
#[derive(Debug)]
pub struct ObjectInfo {
	// The sequence number of the object within the group.
	pub object_id: u64,

	// The size of the object.
	pub size: usize,
}
