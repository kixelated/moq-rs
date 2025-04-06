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

use super::{Frame, FrameConsumer, FrameProducer};

/// An independent group of frames.
#[derive(Clone, PartialEq, Debug)]
pub struct Group {
	// The sequence number of the group within the track.
	// NOTE: These may be received out of order
	pub sequence: u64,
}

impl Group {
	pub fn new(sequence: u64) -> Group {
		Self { sequence }
	}

	pub fn produce(self) -> (GroupProducer, GroupConsumer) {
		let (send, recv) = watch::channel(GroupState::default());

		let writer = GroupProducer::new(send, self.clone());
		let reader = GroupConsumer::new(recv, self);

		(writer, reader)
	}
}

#[derive(Debug)]
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
#[derive(Clone, Debug)]
pub struct GroupProducer {
	// Mutable stream state.
	state: watch::Sender<GroupState>,

	// Immutable stream state.
	pub info: Group,
}

impl GroupProducer {
	fn new(state: watch::Sender<GroupState>, info: Group) -> Self {
		Self { state, info }
	}

	// Write a frame in one go
	pub fn write_frame<B: Into<Bytes>>(&mut self, frame: B) {
		let frame = frame.into();
		self.create_frame(frame.len()).write(frame);
	}

	// Create a frame with an upfront size
	pub fn create_frame(&mut self, size: usize) -> FrameProducer {
		let (writer, reader) = Frame::new(size).produce();
		self.state.send_modify(|state| state.frames.push(reader));
		writer
	}

	pub fn frame_count(&self) -> usize {
		self.state.borrow().frames.len()
	}

	/// Create a new consumer for the group.
	pub fn subscribe(&self) -> GroupConsumer {
		GroupConsumer::new(self.state.subscribe(), self.info.clone())
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
#[derive(Clone, Debug)]
pub struct GroupConsumer {
	// Modify the stream state.
	state: watch::Receiver<GroupState>,

	// Immutable stream state.
	pub info: Group,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,

	// Used to make read_frame cancel safe.
	active: Option<FrameConsumer>,
}

impl GroupConsumer {
	fn new(state: watch::Receiver<GroupState>, group: Group) -> Self {
		Self {
			state,
			info: group,
			index: 0,
			active: None,
		}
	}

	// Read the next frame.
	pub async fn read_frame(&mut self) -> Result<Option<Bytes>, Error> {
		// In order to be cancel safe, we need to save the active frame.
		// That way if this method gets caneclled, we can resume where we left off.
		if self.active.is_none() {
			self.active = match self.next_frame().await? {
				Some(frame) => Some(frame),
				None => return Ok(None),
			};
		};

		// Read the frame in one go, which is cancel safe.
		let frame = self.active.as_mut().unwrap().read_all().await?;
		self.active = None;

		Ok(Some(frame))
	}

	// Return a reader for the next frame.
	pub async fn next_frame(&mut self) -> Result<Option<FrameConsumer>, Error> {
		// Just in case someone called read_frame, cancelled it, then called next_frame.
		if let Some(frame) = self.active.take() {
			return Ok(Some(frame));
		}

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
