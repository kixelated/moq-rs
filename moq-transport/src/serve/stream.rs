use std::{fmt, ops::Deref, sync::Arc};

use crate::util::Watch;

use super::ServeError;

#[derive(Debug)]
pub struct Stream {
	pub namespace: String,
	pub name: String,
	pub send_order: u64,
}

impl Stream {
	pub fn produce(self) -> (StreamPublisher, StreamSubscriber) {
		let state = Watch::new(State::default());
		let info = Arc::new(self);

		let publisher = StreamPublisher::new(state.clone(), info.clone());
		let subscriber = StreamSubscriber::new(state, info);

		(publisher, subscriber)
	}
}

#[derive(Debug)]
struct State {
	// The data that has been received thus far.
	objects: Vec<StreamObject>,

	// Set when the publisher is dropped.
	closed: Result<(), ServeError>,
}

impl State {
	pub fn insert_object(&mut self, object: StreamObject) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.objects.push(object);
		Ok(())
	}

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
pub struct StreamPublisher {
	// Mutable stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Stream>,
}

impl StreamPublisher {
	fn new(state: Watch<State>, info: Arc<Stream>) -> Self {
		Self { state, info }
	}

	/// Create an object with the given info and payload.
	pub fn write_object(&mut self, info: StreamObject) -> Result<(), ServeError> {
		self.state.lock_mut().insert_object(info)?;
		Ok(())
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for StreamPublisher {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a stream has new data available.
#[derive(Clone, Debug)]
pub struct StreamSubscriber {
	// Modify the stream state.
	state: Watch<State>,

	// Immutable stream state.
	info: Arc<Stream>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	_dropped: Arc<Dropped>,
}

impl StreamSubscriber {
	fn new(state: Watch<State>, info: Arc<Stream>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	/// Block until the next object is available.
	pub async fn next(&mut self) -> Result<Option<StreamObject>, ServeError> {
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

	pub fn latest(&self) -> Option<(u64, u64)> {
		self.state
			.lock()
			.objects
			.iter()
			.max_by_key(|a| (a.group_id, a.object_id))
			.map(|a| (a.group_id, a.object_id))
	}
}

impl Deref for StreamSubscriber {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.info
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
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for Dropped {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Dropped").finish()
	}
}

#[derive(Clone)]
pub struct StreamObject {
	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The payload.
	pub payload: bytes::Bytes,
}

impl fmt::Debug for StreamObject {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("StreamObject")
			.field("group_id", &self.group_id)
			.field("object_id", &self.object_id)
			.field("payload", &self.payload.len())
			.finish()
	}
}
