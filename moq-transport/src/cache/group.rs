//! A stream is a stream of objects with a header, split into a [Publisher] and [Subscriber] handle.
//!
//! A [Publisher] writes an ordered stream of objects.
//! Each object can have a sequence number, allowing the subscriber to detect gaps objects.
//!
//! A [Subscriber] reads an ordered stream of objects.
//! The subscriber can be cloned, in which case each subscriber receives a copy of each object. (fanout)
//!
//! The stream is closed with [CacheError::Closed] when all publishers or subscribers are dropped.
use std::{ops::Deref, sync::Arc};

use crate::{error::CacheError, publisher, util::Watch, ServeError};

use super::{ObjectHeader, ObjectPublisher, ObjectSubscriber};

/// Static information about the stream.
#[derive(Debug)]
pub struct Group {
	// The sequence number of the stream within the track.
	// NOTE: These may be received out of order or with gaps.
	pub id: u64,

	// The priority of the stream within the BROADCAST.
	pub send_order: u64,
}

struct GroupState {
	// The data that has been received thus far.
	objects: Vec<ObjectSubscriber>,

	// Set when the publisher is dropped.
	closed: Result<(), CacheError>,
}

impl GroupState {
	pub fn close(&mut self, err: CacheError) -> Result<(), CacheError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for GroupState {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify subscribers.
pub struct GroupPublisher {
	// Mutable stream state.
	state: Watch<GroupState>,

	// Immutable stream state.
	info: Arc<Group>,

	// The next object sequence number to use.
	next: u64,

	// A subscriber
	subscriber: GroupSubscriber,
}

impl GroupPublisher {
	pub fn new(info: Group) -> Self {
		let state = Watch::new(GroupState::default());
		let info = Arc::new(info);
		let subscriber = GroupSubscriber::new(state.clone(), info.clone());

		Self {
			state,
			info,
			next: 0,
			subscriber,
		}
	}

	/// Create the next object ID with the given payload.
	pub fn write_object(&mut self, payload: bytes::Bytes) -> Result<(), CacheError> {
		let mut object = self.create_object(payload.len())?;
		object.write(payload)?;
		Ok(())
	}

	/// Write an object over multiple writes.
	///
	/// BAD STUFF will happen if the size is wrong.
	pub fn create_object(&mut self, size: usize) -> Result<ObjectPublisher, CacheError> {
		let publisher = ObjectPublisher::new(ObjectHeader {
			group_id: self.info.id,
			object_id: self.next.try_into().unwrap(),
			send_order: self.info.send_order,
			size,
		});
		self.next += 1;

		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.objects.push(publisher.subscribe());
		Ok(publisher)
	}

	/// Creates a subscriber for the group with the initial state.
	pub fn subscribe(&self) -> GroupSubscriber {
		self.subscriber.clone()
	}

	/// Close the stream with an error.
	pub fn close(self, err: CacheError) -> Result<(), CacheError> {
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
#[derive(Clone)]
pub struct GroupSubscriber {
	// Modify the stream state.
	state: Watch<GroupState>,

	// Immutable stream state.
	info: Arc<Group>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,
}

impl GroupSubscriber {
	fn new(state: Watch<GroupState>, info: Arc<Group>) -> Self {
		Self { state, info, index: 0 }
	}

	pub fn latest(&self) -> u64 {
		self.state.lock().objects.len() as u64
	}

	/// Block until the next object is available.
	pub async fn object(&mut self) -> Result<ObjectSubscriber, CacheError> {
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

	pub async fn serve(mut self, mut dst: publisher::Subscribe) -> Result<(), ServeError> {
		let mut dst = dst
			.serve_group(publisher::GroupHeader {
				group_id: self.id,
				send_order: self.send_order,
			})
			.await?;

		loop {
			let mut src = self.object().await?;

			dst.write_object(publisher::GroupObject {
				object_id: src.object_id,
				size: src.size,
			})
			.await?;

			while let Some(chunk) = src.chunk().await? {
				dst.write_payload(&chunk).await?;
			}
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
