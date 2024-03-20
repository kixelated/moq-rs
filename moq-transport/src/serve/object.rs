//! A fragment is a stream of bytes with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of bytes in chunks.
//! There's no framing, so these chunks can be of any size or position, and won't be maintained over the network.
//!
//! A [Subscriber] reads an ordered stream of bytes in chunks.
//! These chunks are returned directly from the QUIC connection, so they may be of any size or position.
//! You can clone the [Subscriber] and each will read a copy of of all future chunks. (fanout)
//!
//! The fragment is closed with [ServeError::Closed] when all publishers or subscribers are dropped.
use std::{fmt, ops::Deref, sync::Arc};

use super::ServeError;
use crate::util::Watch;
use bytes::Bytes;

/// Static information about the segment.
#[derive(Clone, Debug)]
pub struct ObjectHeader {
	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The priority of the stream.
	pub send_order: u64,

	// The size of the object
	pub size: usize,
}

impl ObjectHeader {
	pub fn produce(self) -> (ObjectPublisher, ObjectSubscriber) {
		let state = Watch::new(State::default());
		let info = Arc::new(self);

		let publisher = ObjectPublisher::new(state.clone(), info.clone());
		let subscriber = ObjectSubscriber::new(state, info);

		(publisher, subscriber)
	}
}

/// Same as below but with a fully known payload.
#[derive(Clone)]
pub struct Object {
	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The priority of the stream.
	pub send_order: u64,

	// The payload.
	pub payload: Bytes,
}

impl From<Object> for ObjectHeader {
	fn from(info: Object) -> Self {
		Self {
			group_id: info.group_id,
			object_id: info.object_id,
			send_order: info.send_order,
			size: info.payload.len(),
		}
	}
}

impl fmt::Debug for Object {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Object")
			.field("group_id", &self.group_id)
			.field("object_id", &self.object_id)
			.field("send_order", &self.send_order)
			.field("payload", &self.payload.len())
			.finish()
	}
}

struct State {
	// The data that has been received thus far.
	chunks: Vec<Bytes>,

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

impl fmt::Debug for State {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("State")
			.field("chunks", &self.chunks.len())
			.field("size", &self.chunks.iter().map(|c| c.len()).sum::<usize>())
			.field("closed", &self.closed)
			.finish()
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a segment and notify subscribers.
#[derive(Debug)]
pub struct ObjectPublisher {
	// Mutable segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<ObjectHeader>,

	// The amount of promised data that has yet to be written.
	remain: usize,
}

impl ObjectPublisher {
	/// Create a new segment with the given info.
	fn new(state: Watch<State>, info: Arc<ObjectHeader>) -> Self {
		Self {
			state,
			remain: info.size,
			info,
		}
	}

	/// Write a new chunk of bytes.
	pub fn write(&mut self, chunk: Bytes) -> Result<(), ServeError> {
		if chunk.len() > self.remain {
			return Err(ServeError::WrongSize);
		}
		self.remain -= chunk.len();

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.chunks.push(chunk);

		Ok(())
	}

	/// Close the segment with an error.
	pub fn close(&mut self, mut err: ServeError) -> Result<(), ServeError> {
		if err == ServeError::Done && self.remain != 0 {
			err = ServeError::WrongSize;
		}

		self.state.lock_mut().close(err)?;
		Ok(())
	}
}

impl Drop for ObjectPublisher {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
	}
}

impl Deref for ObjectPublisher {
	type Target = ObjectHeader;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a segment has new data available.
#[derive(Clone, Debug)]
pub struct ObjectSubscriber {
	// Modify the segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<ObjectHeader>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	_dropped: Arc<Dropped>,
}

impl ObjectSubscriber {
	fn new(state: Watch<State>, info: Arc<ObjectHeader>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	/// Block until the next chunk of bytes is available.
	pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.index < state.chunks.len() {
					let chunk = state.chunks[self.index].clone();
					self.index += 1;
					return Ok(Some(chunk));
				}

				match &state.closed {
					Err(ServeError::Done) => return Ok(None),
					Err(err) => return Err(err.clone()),
					Ok(()) => state.changed(),
				}
			};

			notify.await; // Try again when the state changes
		}
	}

	pub async fn read_all(&mut self) -> Result<Bytes, ServeError> {
		let mut chunks = Vec::new();
		while let Some(chunk) = self.read().await? {
			chunks.push(chunk);
		}

		Ok(Bytes::from(chunks.concat()))
	}
}

impl Deref for ObjectSubscriber {
	type Target = ObjectHeader;

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
