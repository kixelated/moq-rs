use bytes::{Bytes, BytesMut};
use std::fmt;
use tokio::sync::watch;

use crate::Error;

#[derive(Clone, Debug)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct Frame {
	pub size: u64,
}

impl Frame {
	pub fn new(size: u64) -> Self {
		Self { size }
	}

	pub fn produce(self) -> FrameProducer {
		FrameProducer::new(self)
	}
}

impl From<usize> for Frame {
	fn from(size: usize) -> Self {
		Self::new(size as u64)
	}
}

impl From<u64> for Frame {
	fn from(size: u64) -> Self {
		Self::new(size)
	}
}

impl From<u32> for Frame {
	fn from(size: u32) -> Self {
		Self::new(size as u64)
	}
}

impl From<u16> for Frame {
	fn from(size: u16) -> Self {
		Self::new(size as u64)
	}
}

struct FrameState {
	// The chunks that has been written thus far
	chunks: Vec<Bytes>,

	// Set when the writer or all readers are dropped.
	closed: Result<(), Error>,
}

impl Default for FrameState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

impl fmt::Debug for FrameState {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("FrameState")
			.field("chunks", &self.chunks.len())
			.field("closed", &self.closed)
			.finish()
	}
}

/// Used to write a frame's worth of data in chunks.
#[derive(Clone, Debug)]
pub struct FrameProducer {
	// Immutable stream state.
	pub info: Frame,

	// Mutable stream state.
	state: watch::Sender<FrameState>,
}

impl FrameProducer {
	pub fn new(info: Frame) -> Self {
		Self {
			info,
			state: Default::default(),
		}
	}

	pub fn write<B: Into<Bytes>>(&mut self, chunk: B) {
		self.state.send_modify(|state| state.chunks.push(chunk.into()));
	}

	/// Close the stream with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| state.closed = Err(err));
	}

	/// Create a new consumer for the frame.
	pub fn consume(&self) -> FrameConsumer {
		FrameConsumer {
			info: self.info.clone(),
			state: self.state.subscribe(),
			index: 0,
		}
	}

	pub async fn unused(&self) {
		self.state.closed().await;
	}
}

impl From<Frame> for FrameProducer {
	fn from(info: Frame) -> Self {
		FrameProducer::new(info)
	}
}

/// Used to consume a frame's worth of data in chunks.
#[derive(Clone, Debug)]
pub struct FrameConsumer {
	// Immutable stream state.
	pub info: Frame,

	// Modify the stream state.
	state: watch::Receiver<FrameState>,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,
}

impl FrameConsumer {
	// Return the next chunk.
	pub async fn read(&mut self) -> Result<Option<Bytes>, Error> {
		loop {
			{
				let state = self.state.borrow_and_update();

				if let Some(chunk) = state.chunks.get(self.index).cloned() {
					self.index += 1;
					return Ok(Some(chunk));
				}

				state.closed.clone()?;
			}

			if self.state.changed().await.is_err() {
				return Ok(None);
			}
		}
	}

	// Return all of the remaining chunks concatenated together.
	pub async fn read_all(&mut self) -> Result<Bytes, Error> {
		// Wait until the writer is done before even attempting to read.
		// That way this function can be cancelled without consuming half of the frame.
		if let Ok(err) = self.state.wait_for(|s| s.closed.is_err()).await {
			return Err(err.closed.clone().unwrap_err());
		};

		// Get all of the remaining chunks.
		let state = self.state.borrow_and_update();
		let chunks = &state.chunks[self.index..];
		self.index = state.chunks.len();

		// We know the final size so we can allocate the buffer upfront.
		let size = chunks.iter().map(Bytes::len).sum();
		let mut buf = BytesMut::with_capacity(size);

		// Copy the chunks into the buffer.
		for chunk in chunks {
			buf.extend_from_slice(chunk);
		}

		Ok(buf.freeze())
	}

	pub async fn closed(&self) -> Result<(), Error> {
		match self.state.clone().wait_for(|state| state.closed.is_err()).await {
			Ok(state) => state.closed.clone(),
			Err(_) => Ok(()),
		}
	}
}
