use crate::util::State;
use bytes::{Bytes, BytesMut};
use std::{ops, sync::Arc};

use super::Closed;

#[derive(Clone, PartialEq)]
pub struct Frame {
	pub size: usize,
}

impl Frame {
	pub fn new(size: usize) -> Frame {
		Self { size }
	}

	pub fn produce(self) -> (FrameWriter, FrameReader) {
		let state = State::default();
		let info = Arc::new(self);

		let writer = FrameWriter::new(state.split(), info.clone());
		let reader = FrameReader::new(state, info);

		(writer, reader)
	}
}

struct FrameState {
	// The chunks that has been written thus far
	chunks: Vec<Bytes>,

	// Set when the writer or all readers are dropped.
	closed: Result<(), Closed>,
}

impl Default for FrameState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify readers.
pub struct FrameWriter {
	// Mutable stream state.
	state: State<FrameState>,

	// Immutable stream state.
	pub info: Arc<Frame>,
}

impl FrameWriter {
	fn new(state: State<FrameState>, info: Arc<Frame>) -> Self {
		Self { state, info }
	}

	pub fn write(&mut self, chunk: bytes::Bytes) -> Result<(), Closed> {
		let mut state = self.state.lock_mut().ok_or(Closed::Cancel)?;
		state.chunks.push(chunk);

		Ok(())
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: Closed) -> Result<(), Closed> {
		let state = self.state.lock();
		state.closed.clone()?;
		state.into_mut().ok_or(Closed::Cancel)?.closed = Err(err);

		Ok(())
	}
}

impl ops::Deref for FrameWriter {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a stream has new data available.
#[derive(Clone)]
pub struct FrameReader {
	// Modify the stream state.
	state: State<FrameState>,

	// Immutable stream state.
	pub info: Arc<Frame>,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,
}

impl FrameReader {
	fn new(state: State<FrameState>, group: Arc<Frame>) -> Self {
		Self {
			state,
			info: group,
			index: 0,
		}
	}

	// Return the next chunk.
	pub async fn read(&mut self) -> Result<Option<Bytes>, Closed> {
		loop {
			{
				let state = self.state.lock();

				if let Some(chunk) = state.chunks.get(self.index).cloned() {
					self.index += 1;
					return Ok(Some(chunk));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(modified) => modified,
					None => return Ok(None),
				}
			}
			.await; // Try again when the state changes
		}
	}

	// Return all of the chunks concatenated together.
	pub async fn read_all(&mut self) -> Result<Bytes, Closed> {
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
}

impl ops::Deref for FrameReader {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
