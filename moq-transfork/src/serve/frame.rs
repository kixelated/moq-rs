use crate::util::State;
use bytes::{Bytes, BytesMut};
use std::{ops::Deref, sync::Arc};

use super::ServeError;

#[derive(Debug, Clone, PartialEq)]
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
	closed: Result<(), ServeError>,
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

	pub fn write(&mut self, chunk: bytes::Bytes) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.chunks.push(chunk);
		Ok(())
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Deref for FrameWriter {
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
	pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
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
	pub async fn read_all(&mut self) -> Result<Bytes, ServeError> {
		let first = self.read().await?.unwrap_or_else(|| Bytes::new());
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

impl Deref for FrameReader {
	type Target = Frame;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/*
pub struct Frame {
	// The size of the frame.
	pub size: usize,
}

pub struct FrameState {
	size:
}
	// Mutable stream state.
	state: State<FrameState>,

	// Immutable stream state.
	pub info: Arc<Frame>,
}

pub struct FrameWriter {
	group: FrameWriter,
}

impl FrameWriter {
	pub(super) fn new(group: FrameWriter) -> Self {
		Self { group }
	}

	pub fn write(&mut self, chunk: bytes::Bytes) -> Result<(), ServeError> {
		// TODO figure out a way to avoid allocating on the heap
		let mut buf = BytesMut::with_capacity(8);
		chunk.len().encode(&mut buf).unwrap();

		self.group.write(buf.freeze())?;
		self.group.write(chunk)
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.group.close(err)
	}
}

impl Deref for FrameWriter {
	type Target = FrameWriter;

	fn deref(&self) -> &Self::Target {
		&self.group
	}
}

#[derive(Clone)]
pub struct FrameReader {
	group: FrameReader,

	next: Option<usize>,
	buf: BytesMut,
}

impl FrameReader {
	pub(super) fn new(group: FrameReader) -> Self {
		Self {
			group,
			next: None,
			buf: BytesMut::new(),
		}
	}

	pub async fn next(&mut self) -> Result<Option<Bytes>, ServeError> {
		loop {
			if self.next.is_none() {
				let mut cursor = io::Cursor::new(&self.buf);
				match usize::decode(&mut cursor) {
					Ok(size) => {
						self.next = Some(size);
						self.buf.advance(cursor.position() as usize);
					}
					Err(DecodeError::More(_)) => {}
					Err(err) => unreachable!("unexpected error: {:?}", err),
				}
			}

			if let Some(size) = self.next {
				if self.buf.len() >= size {
					let chunk = self.buf.split_to(size).freeze();
					self.next = None;
					return Ok(Some(chunk));
				}
			}

			match self.group.next().await? {
				None if self.buf.is_empty() && self.next.is_none() => return Ok(None),
				None => return Err(ServeError::Size),
				Some(chunk) => self.buf.extend_from_slice(&chunk),
			}
		}
	}
}

impl Deref for FrameReader {
	type Target = FrameReader;

	fn deref(&self) -> &Self::Target {
		&self.group
	}
}
*/
