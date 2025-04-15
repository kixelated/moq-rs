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
use tokio::sync::watch;

use crate::Error;

use super::{Frame, FrameConsumer, FrameProducer};

#[derive(Clone, Debug, Hash, Eq, PartialEq, Ord, PartialOrd)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Group {
	pub sequence: u64,
}

impl Group {
	pub fn new(sequence: u64) -> Self {
		Self { sequence }
	}

	pub fn produce(self) -> GroupProducer {
		GroupProducer::new(self)
	}
}

impl From<usize> for Group {
	fn from(sequence: usize) -> Self {
		Self::new(sequence as u64)
	}
}

impl From<u64> for Group {
	fn from(sequence: u64) -> Self {
		Self::new(sequence)
	}
}

impl From<u32> for Group {
	fn from(sequence: u32) -> Self {
		Self::new(sequence as u64)
	}
}

impl From<u16> for Group {
	fn from(sequence: u16) -> Self {
		Self::new(sequence as u64)
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
#[derive(Clone)]
pub struct GroupProducer {
	// Mutable stream state.
	state: watch::Sender<GroupState>,

	// Immutable stream state.
	pub info: Group,
}

impl GroupProducer {
	pub fn new(info: Group) -> Self {
		Self {
			info,
			state: Default::default(),
		}
	}

	/// A helper method to write a frame from a single byte buffer.
	///
	/// If you want to write multiple chunks, use [Self::create_frame] or [Self::append_frame].
	/// But an upfront size is required.
	pub fn write_frame<B: Into<Bytes>>(&mut self, frame: B) {
		let data = frame.into();
		let frame = Frame::new(data.len() as u64);
		self.create_frame(frame).write(data);
	}

	/// Create a frame with an upfront size
	pub fn create_frame(&mut self, info: Frame) -> FrameProducer {
		let producer = FrameProducer::new(info);
		self.append_frame(producer.consume());
		producer
	}

	/// Append a frame to the group.
	pub fn append_frame(&mut self, consumer: FrameConsumer) {
		self.state.send_modify(|state| state.frames.push(consumer));
	}

	/// Create a new consumer for the group.
	pub fn consume(&self) -> GroupConsumer {
		GroupConsumer {
			info: self.info.clone(),
			state: self.state.subscribe(),
			index: 0,
			active: None,
		}
	}

	pub async fn unused(&self) {
		self.state.closed().await;
	}

	/// Close the stream with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| {
			state.closed = Err(err);
		});
	}
}

impl From<Group> for GroupProducer {
	fn from(info: Group) -> Self {
		GroupProducer::new(info)
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

	// Used to make read_frame cancel safe.
	active: Option<FrameConsumer>,
}

impl GroupConsumer {
	/// Read the next frame.
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

	/// Return a reader for the next frame.
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

	/// Block until the group is closed and return the error.
	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}
