use std::{ops::Deref, sync::Arc};

use crate::{publisher, util::Watch, CacheError, ServeError};

use super::Object;

pub struct Stream {
	pub namespace: String,
	pub name: String,
	pub send_order: u64,
}
struct StreamState {
	// The data that has been received thus far.
	objects: Vec<Object>,

	// Set when the publisher is dropped.
	closed: Result<(), CacheError>,
}

impl StreamState {
	pub fn insert_object(&mut self, object: Object) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.objects.push(object);
		Ok(())
	}

	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for StreamState {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify subscribers.
pub struct StreamPublisher {
	// Mutable stream state.
	state: Watch<StreamState>,

	// Immutable stream state.
	info: Arc<Stream>,

	// A subscriber
	subscriber: StreamSubscriber,
}

impl StreamPublisher {
	pub fn new(info: Stream) -> Self {
		let state = Watch::new(StreamState::default());
		let info = Arc::new(info);
		let subscriber = StreamSubscriber::new(state.clone(), info.clone());

		Self {
			state,
			info,
			subscriber,
		}
	}

	/// Create an object with the given info and payload.
	pub fn write_object(&mut self, info: Object) -> Result<(), CacheError> {
		self.state.lock_mut().insert_object(info)?;
		Ok(())
	}

	/// Creates a subscriber for the group with the initial state.
	pub fn subscribe(&self) -> StreamSubscriber {
		self.subscriber.clone()
	}

	/// Close the stream with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
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
#[derive(Clone)]
pub struct StreamSubscriber {
	// Modify the stream state.
	state: Watch<StreamState>,

	// Immutable stream state.
	info: Arc<Stream>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,
}

impl StreamSubscriber {
	fn new(state: Watch<StreamState>, info: Arc<Stream>) -> Self {
		Self { state, info, index: 0 }
	}

	/// Block until the next object is available.
	pub async fn object(&mut self) -> Result<Object, CacheError> {
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

	pub fn latest(&self) -> Option<(u64, u64)> {
		self.state
			.lock()
			.objects
			.iter()
			.max_by_key(|a| (a.group_id, a.object_id))
			.map(|a| (a.group_id, a.object_id))
	}

	pub async fn serve(mut self, mut dst: publisher::Subscribe) -> Result<(), ServeError> {
		let mut dst = dst
			.serve_track(publisher::TrackHeader {
				send_order: self.send_order,
			})
			.await?;

		loop {
			// TODO add ability to read one chunk at a time
			let object = self.object().await?;

			dst.write_object(publisher::TrackObject {
				group_id: object.group_id,
				object_id: object.object_id,
				size: object.payload.len(),
			})
			.await?;

			dst.write_payload(&object.payload).await?;
		}
	}
}

impl Deref for StreamSubscriber {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
