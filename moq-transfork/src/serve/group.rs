//! A group is a stream of frames, split into a [Writer] and [Reader] handle.
//!
//! A [Writer] writes an ordered stream of frames.
//! Frames can be written all at once, or in chunks.
//!
//! A [Reader] reads an ordered stream of frames.
//! The reader can be cloned, in which case each reader receives a copy of each frame. (fanout)
//!
//! The stream is closed with [ServeError::Closed] when all writers or readers are dropped.
use bytes::Bytes;
use std::{ops::Deref, sync::Arc, time};

use crate::util::State;

use super::{Frame, FrameReader, FrameWriter, ServeError};

/// Parameters that can be specified by the user
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
	// The sequence number of the group within the track.
	// NOTE: These may be received out of order
	pub sequence: u64,

	// The amount of time until this group expires, starting after the next group.
	pub expires: time::Duration,
}

impl Group {
	pub fn new(sequence: u64) -> GroupBuilder {
		GroupBuilder::new(Self {
			sequence,
			expires: time::Duration::ZERO,
		})
	}

	pub fn produce(self) -> (GroupWriter, GroupReader) {
		let state = State::default();
		let info = Arc::new(self);

		let writer = GroupWriter::new(state.split(), info.clone());
		let reader = GroupReader::new(state, info);

		(writer, reader)
	}
}

pub struct GroupBuilder {
	group: Group,
}

impl GroupBuilder {
	fn new(group: Group) -> Self {
		Self { group }
	}

	pub fn expires(mut self, expires: time::Duration) -> Self {
		self.group.expires = expires;
		self
	}

	pub fn build(self) -> Group {
		self.group
	}

	pub fn produce(self) -> (GroupWriter, GroupReader) {
		self.build().produce()
	}
}

struct GroupState {
	// The frames that has been written thus far
	frames: Vec<FrameReader>,

	// Set when the writer or all readers are dropped.
	closed: Result<(), ServeError>,
}

impl Default for GroupState {
	fn default() -> Self {
		Self {
			frames: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify readers.
pub struct GroupWriter {
	// Mutable stream state.
	state: State<GroupState>,

	// Immutable stream state.
	pub info: Arc<Group>,

	// Cache the number of frames we've written to avoid a mutex
	total: usize,
}

impl GroupWriter {
	fn new(state: State<GroupState>, info: Arc<Group>) -> Self {
		Self { state, info, total: 0 }
	}

	// Write a frame in one go
	pub fn write(&mut self, frame: bytes::Bytes) -> Result<(), ServeError> {
		self.write_chunks(frame.len())?.write(frame)
	}

	// Create a frame with an upfront size
	pub fn write_chunks(&mut self, size: usize) -> Result<FrameWriter, ServeError> {
		let (writer, reader) = Frame::new(size).produce();
		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.frames.push(reader);
		self.total += 1;
		Ok(writer)
	}

	/// Close the stream with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}

	pub fn total(&self) -> usize {
		self.total
	}
}

impl Deref for GroupWriter {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a stream has new data available.
#[derive(Clone)]
pub struct GroupReader {
	// Modify the stream state.
	state: State<GroupState>,

	// Immutable stream state.
	pub info: Arc<Group>,

	// The number of frames we've read.
	// NOTE: Cloned readers inherit this offset, but then run in parallel.
	index: usize,
}

impl GroupReader {
	fn new(state: State<GroupState>, group: Arc<Group>) -> Self {
		Self {
			state,
			info: group,
			index: 0,
		}
	}

	// Read the next frame.
	pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
		Ok(match self.read_chunks().await? {
			Some(mut reader) => Some(reader.read_all().await?),
			None => None,
		})
	}

	// Return a reader for the next frame.
	pub async fn read_chunks(&mut self) -> Result<Option<FrameReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();

				if let Some(frame) = state.frames.get(self.index).cloned() {
					self.index += 1;
					return Ok(Some(frame));
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

	// Return the current index of the frame in the group
	pub fn current(&self) -> usize {
		self.index
	}

	// Return the current total number of frames in the group
	pub fn total(&self) -> usize {
		self.state.lock().frames.len()
	}
}

impl Deref for GroupReader {
	type Target = Group;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
