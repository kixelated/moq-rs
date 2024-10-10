use bytes::{Bytes, BytesMut};
use std::ops;
use tokio::sync::watch;

use crate::{Error, Produce};

/// A frame of data with an upfront size.
#[derive(Clone, PartialEq)]
pub struct Frame {
	pub size: usize,
}

impl Frame {
	pub fn new(size: usize) -> Frame {
		Self { size }
	}
}

impl Produce for Frame {
	type Consumer = FrameConsumer;
	type Producer = FrameProducer;

	fn produce(self) -> (FrameProducer, FrameConsumer) {
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

/// Used to write a frame's worth of data in chunks.
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

	pub fn write(&mut self, chunk: bytes::Bytes) {
		self.state.send_modify(|state| state.chunks.push(chunk));
	}

	/// Close the stream with an error.
	pub fn close(self, err: Error) {
		self.state.send_modify(|state| state.closed = Err(err));
	}
}

impl ops::Deref for FrameProducer {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Used to consume a frame's worth of data in chunks.
#[derive(Clone)]
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

	// Return all of the chunks concatenated together.
	pub async fn read_all(&mut self) -> Result<Bytes, Error> {
		let first = self.read().await?.unwrap_or_else(Bytes::new);
		if first.len() == self.size {
			// If there's one chunk, return it without allocating.
			return Ok(first);
		}

		let mut buf = BytesMut::with_capacity(2 * first.len());
		buf.extend_from_slice(&first);

		while let Some(chunk) = self.read().await? {
			buf.extend_from_slice(&chunk);
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
