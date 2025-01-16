use bytes::{Bytes, BytesMut};
use std::{fmt, ops};
use tokio::sync::watch;

use crate::Error;

/// A frame of data with an upfront size.
#[derive(Clone, PartialEq, Debug)]
pub struct Frame {
	pub size: usize,
}

impl Frame {
	pub fn new(size: usize) -> Frame {
		Self { size }
	}

	pub fn produce(self) -> (FrameProducer, FrameConsumer) {
		let (send, recv) = watch::channel(FrameState::default());

		let writer = FrameProducer::new(send, self.clone());
		let reader = FrameConsumer::new(recv, self);

		(writer, reader)
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
	// Mutable stream state.
	state: watch::Sender<FrameState>,

	// Immutable stream state.
	pub info: Frame,
}

impl FrameProducer {
	fn new(state: watch::Sender<FrameState>, info: Frame) -> Self {
		Self { state, info }
	}

	pub fn write<B: Into<Bytes>>(&mut self, chunk: B) {
		self.state.send_modify(|state| state.chunks.push(chunk.into()));
	}

	/// Close the stream with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| state.closed = Err(err));
	}

	/// Create a new consumer for the frame.
	pub fn subscribe(&self) -> FrameConsumer {
		FrameConsumer::new(self.state.subscribe(), self.info.clone())
	}
}

impl ops::Deref for FrameProducer {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Used to consume a frame's worth of data in chunks.
#[derive(Clone, Debug)]
pub struct FrameConsumer {
	// Modify the stream state.
	state: watch::Receiver<FrameState>,

	// Immutable stream state.
	pub info: Frame,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,
}

impl FrameConsumer {
	fn new(state: watch::Receiver<FrameState>, group: Frame) -> Self {
		Self {
			state,
			info: group,
			index: 0,
		}
	}

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

impl ops::Deref for FrameConsumer {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
