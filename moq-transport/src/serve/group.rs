//! A stream is a stream of objects with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of objects.
//! Each object can have a sequence number, allowing the subscriber to detect gaps objects.
//!
//! A [Subscriber] reads an ordered stream of objects.
//! The subscriber can be cloned, in which case each subscriber receives a copy of each object. (fanout)
//!
//! The stream is closed with [ServeError::Closed] when all publishers or subscribers are dropped.
use std::{fmt, ops::Deref, sync::Arc};

use crate::util::Watch;

use super::{ObjectHeader, ObjectPublisher, ObjectSubscriber, ServeError};

/// Static information about the stream.
#[derive(Debug)]
pub struct Group {
	// The sequence number of the stream within the track.
	// NOTE: These may be received out of order or with gaps.
	pub id: u64,

	// The priority of the stream within the BROADCAST.
	pub send_order: u64,
}

impl Group {
	pub fn produce(self) -> (GroupPublisher, GroupSubscriber) {
		let state = Watch::new(State::default());
		let info = Arc::new(self);

		let publisher = GroupPublisher::new(state.clone(), info.clone());
		let subscriber = GroupSubscriber::new(state, info);

		(publisher, subscriber)
	}
}

#[derive(Debug)]
struct State {
	// The data that has been received thus far.
	objects: Vec<ObjectSubscriber>,

	// Set when the publisher is dropped.
	closed: Result<(), ServeError>,
}

impl State {
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
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

/// Used to write data to a stream and notify subscribers.
#[derive(Debug)]
pub struct GroupPublisher {
	// Mutable stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Group>,

	// The next object sequence number to use.
	next: u64,
}

impl GroupPublisher {
	fn new(state: Watch<State>, info: Arc<Group>) -> Self {
		Self { state, info, next: 0 }
	}

	/// Create the next object ID with the given payload.
	pub fn write_object(&mut self, payload: bytes::Bytes) -> Result<(), ServeError> {
		let mut object = self.create_object(payload.len())?;
		object.write(payload)?;
		Ok(())
	}

	/// Write an object over multiple writes.
	///
	/// BAD STUFF will happen if the size is wrong.
	pub fn create_object(&mut self, size: usize) -> Result<ObjectPublisher, ServeError> {
		let (publisher, subscriber) = ObjectHeader {
			group_id: self.id,
			object_id: self.next,
			send_order: self.send_order,
			size,
		}
		.produce();

		self.next += 1;

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.objects.push(subscriber);
		Ok(publisher)
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for GroupPublisher {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a stream has new data available.
#[derive(Clone, Debug)]
pub struct GroupSubscriber {
	// Modify the stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Group>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	_dropped: Arc<Dropped>,
}

impl GroupSubscriber {
	fn new(state: Watch<State>, info: Arc<Group>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	pub fn latest(&self) -> u64 {
		self.state.lock().objects.len() as u64
	}

	/// Block until the next object is available.
	pub async fn next(&mut self) -> Result<Option<ObjectSubscriber>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.index < state.objects.len() {
					let object = state.objects[self.index].clone();
					self.index += 1;
					return Ok(Some(object));
				}

				match &state.closed {
					Ok(()) => state.changed(),
					Err(ServeError::Done) => return Ok(None),
					Err(err) => return Err(err.clone()),
				}
			};

			notify.await; // Try again when the state changes
		}
	}
}

impl Deref for GroupSubscriber {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// A subset of Object, since we use the group's info.
#[derive(Debug)]
pub struct GroupObject {
	// The sequence number of the object within the group.
	pub object_id: u64,

	// The size of the object.
	pub size: usize,
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
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for Dropped {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Dropped").finish()
	}
}
