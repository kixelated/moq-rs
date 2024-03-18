//! A fragment is a stream of bytes with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of bytes in chunks.
//! There's no framing, so these chunks can be of any size or position, and won't be maintained over the network.
//!
//! A [Subscriber] reads an ordered stream of bytes in chunks.
//! These chunks are returned directly from the QUIC connection, so they may be of any size or position.
//! You can clone the [Subscriber] and each will read a copy of of all future chunks. (fanout)
//!
//! The fragment is closed with [CacheError::Closed] when all publishers or subscribers are dropped.
use std::{ops::Deref, sync::Arc};

use crate::{error::CacheError, publisher, util::Watch, ServeError};
use bytes::Bytes;

/// Static information about the segment.
#[derive(Clone)]
pub struct ObjectHeader {
	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The priority of the stream.
	pub send_order: u64,

	// The size of the object.
	pub size: usize,
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

struct ObjectState {
	// The data that has been received thus far.
	chunks: Vec<Bytes>,

	// Set when the publisher is dropped.
	closed: Result<(), CacheError>,
}

impl ObjectState {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for ObjectState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a segment and notify subscribers.
pub struct ObjectPublisher {
	// Mutable segment state.
	state: Watch<ObjectState>,

	// Immutable segment state.
	info: Arc<ObjectHeader>,

	// The amount of promised data that has yet to be written.
	remain: usize,

	subscriber: ObjectSubscriber,
}

impl ObjectPublisher {
	/// Create a new segment with the given info.
	pub fn new(info: ObjectHeader) -> Self {
		let state = Watch::new(ObjectState::default());
		let info = Arc::new(info);
		let subscriber = ObjectSubscriber::new(state.clone(), info.clone());

		Self {
			state,
			info,
			remain: 0,
			subscriber,
		}
	}

	/// Write a new chunk of bytes.
	pub fn write(&mut self, chunk: Bytes) -> Result<(), CacheError> {
		if chunk.len() > self.remain {
			return Err(CacheError::WrongSize);
		}
		self.remain -= chunk.len();

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.chunks.push(chunk);

		Ok(())
	}

	pub fn subscribe(&self) -> ObjectSubscriber {
		self.subscriber.clone()
	}

	/// Close the segment with an error.
	pub fn close(self, mut err: CacheError) -> Result<(), CacheError> {
		if err == CacheError::Done && self.remain != 0 {
			err = CacheError::WrongSize;
		}

		self.state.lock_mut().close(err)?;
		Ok(())
	}
}

impl Deref for ObjectPublisher {
	type Target = ObjectHeader;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a segment has new data available.
#[derive(Clone)]
pub struct ObjectSubscriber {
	// Modify the segment state.
	state: Watch<ObjectState>,

	// Immutable segment state.
	info: Arc<ObjectHeader>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,
}

impl ObjectSubscriber {
	fn new(state: Watch<ObjectState>, info: Arc<ObjectHeader>) -> Self {
		Self { state, info, index: 0 }
	}

	/// Block until the next chunk of bytes is available.
	pub async fn chunk(&mut self) -> Result<Option<Bytes>, CacheError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.index < state.chunks.len() {
					let chunk = state.chunks[self.index].clone();
					self.index += 1;
					return Ok(Some(chunk));
				}

				match &state.closed {
					Err(CacheError::Done) => return Ok(None),
					Err(err) => return Err(err.clone()),
					Ok(()) => state.changed(),
				}
			};

			notify.await; // Try again when the state changes
		}
	}

	pub async fn serve(mut self, mut dst: publisher::Subscribe) -> Result<(), ServeError> {
		let mut dst = dst
			.serve_object(publisher::ObjectHeader {
				group_id: self.group_id,
				object_id: self.object_id,
				send_order: self.send_order,
			})
			.await?;

		while let Some(chunk) = self.chunk().await? {
			dst.write(&chunk).await?;
		}

		Ok(())
	}
}

impl Deref for ObjectSubscriber {
	type Target = ObjectHeader;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
