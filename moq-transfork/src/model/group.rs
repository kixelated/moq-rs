//! A group is a stream of frames, split into a [Producer] and [Consumer] handle.
//!
//! A [Producer] writes an ordered stream of frames.
//! Frames can be written all at once, or in chunks.
//!
//! A [Consumer] reads an ordered stream of frames.
//! The reader can be cloned, in which case each reader receives a copy of each frame. (fanout)
//!
//! The stream is closed with [ServeError::MoqError] when all writers or readers are dropped.
use bytes::Bytes;
use std::ops;
use tokio::sync::watch;

use crate::Error;

use super::{Frame, FrameConsumer, FrameProducer, Produce};

/// An independent group of frames.
#[derive(Clone, PartialEq)]
pub struct Group {
	// The sequence number of the group within the track.
	// NOTE: These may be received out of order
	pub sequence: u64,
}

impl Group {
	pub fn new(sequence: u64) -> Group {
		Self { sequence }
	}
}

impl Produce for Group {
	type Consumer = GroupConsumer;
	type Producer = GroupProducer;

	fn produce(self) -> (GroupProducer, GroupConsumer) {
		let (send, recv) = watch::channel(GroupState::default());

		let writer = GroupProducer::new(send, self.clone());
		let reader = GroupConsumer::new(recv, self);

		(writer, reader)
	}
}

struct GroupState {
	// The frames that has been written thus far
	frames: Vec<FrameConsumer>,

	// Set when the writer or all readers are dropped.
	closed: Result<(), Error>,
}

impl Default for GroupState {
	fn default() -> Self {
		Self {
			frames: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Create a group, frame-by-frame.
pub struct GroupProducer {
	// Mutable stream state.
	state: watch::Sender<GroupState>,

	// Immutable stream state.
	pub info: Group,

	// Cache the number of frames we've written to avoid a mutex
	total: usize,
}

impl GroupProducer {
	fn new(state: watch::Sender<GroupState>, info: Group) -> Self {
		Self { state, info, total: 0 }
	}

	// Write a frame in one go
	pub fn write_frame(&mut self, frame: bytes::Bytes) {
		self.create_frame(frame.len()).write(frame);
	}

	// Create a frame with an upfront size
	pub fn create_frame(&mut self, size: usize) -> FrameProducer {
		let (writer, reader) = Frame::new(size).produce();

		self.state.send_modify(|state| state.frames.push(reader));
		self.total += 1;

		writer
	}

	pub fn frame_count(&self) -> usize {
		self.total
	}

	/// Close the stream with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}
}

impl ops::Deref for GroupProducer {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Consume a group, frame-by-frame.
#[derive(Clone)]
pub struct GroupConsumer {
	// Modify the stream state.
	state: watch::Receiver<GroupState>,

	// Immutable stream state.
	pub info: Group,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,
}

impl GroupConsumer {
	fn new(state: watch::Receiver<GroupState>, group: Group) -> Self {
		Self {
			state,
			info: group,
			index: 0,
		}
	}

	// Read the next frame.
	pub async fn read_frame(&mut self) -> Result<Option<Bytes>, Error> {
		Ok(match self.next_frame().await? {
			Some(mut reader) => Some(reader.read_all().await?),
			None => None,
		})
	}

	// Return a reader for the next frame.
	pub async fn next_frame(&mut self) -> Result<Option<FrameConsumer>, Error> {
		loop {
			{
				let state = self.state.borrow_and_update();

				if let Some(frame) = state.frames.get(self.index).cloned() {
					self.index += 1;
					return Ok(Some(frame));
				}

				state.closed.clone()?;
			}

			if self.state.changed().await.is_err() {
				return Ok(None);
			}
		}
	}

	// Return the current index of the frame in the group
	pub fn frame_index(&self) -> usize {
		self.index
	}

	// Return the current total number of frames in the group
	pub fn frame_count(&self) -> usize {
		self.state.borrow().frames.len()
	}

	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}

impl ops::Deref for GroupConsumer {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
