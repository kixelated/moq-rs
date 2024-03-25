//! A fragment is a stream of bytes with a header, split into a [Writer] and [Reader] handle.
//!
//! A [Writer] writes an ordered stream of bytes in chunks.
//! There's no framing, so these chunks can be of any size or position, and won't be maintained over the network.
//!
//! A [Reader] reads an ordered stream of bytes in chunks.
//! These chunks are returned directly from the QUIC connection, so they may be of any size or position.
//! You can clone the [Reader] and each will read a copy of of all future chunks. (fanout)
//!
//! The fragment is closed with [ServeError::Closed] when all writers or readers are dropped.
use std::{cmp, fmt, ops::Deref, sync::Arc};

use super::{ServeError, Track};
use crate::util::Watch;
use bytes::Bytes;

pub struct Objects {
	pub track: Arc<Track>,
}

impl Objects {
	pub fn produce(self) -> (ObjectsWriter, ObjectsReader) {
		let state = Watch::new(ObjectsState::default());

		let writer = ObjectsWriter {
			state: state.clone(),
			track: self.track.clone(),
		};
		let reader = ObjectsReader::new(state, self.track);

		(writer, reader)
	}
}

#[derive(Debug)]
struct ObjectsState {
	// The latest group.
	objects: Vec<ObjectReader>,

	// Increased each time objects changes.
	epoch: usize,

	// Set when the writer or all readers are dropped.
	closed: Result<(), ServeError>,
}

impl ObjectsState {
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for ObjectsState {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			epoch: 0,
			closed: Ok(()),
		}
	}
}

#[derive(Debug)]
pub struct ObjectsWriter {
	state: Watch<ObjectsState>,
	pub track: Arc<Track>,
}

impl ObjectsWriter {
	pub fn write(&mut self, object: Object, payload: Bytes) -> Result<(), ServeError> {
		let mut writer = self.create(object)?;
		writer.write(payload)?;
		Ok(())
	}

	pub fn create(&mut self, object: Object) -> Result<ObjectWriter, ServeError> {
		let object = ObjectInfo {
			track: self.track.clone(),
			group_id: object.group_id,
			object_id: object.object_id,
			priority: object.priority,
		};

		let (writer, reader) = object.produce();

		let mut state = self.state.lock_mut();

		if let Some(first) = state.objects.first() {
			match writer.group_id.cmp(&first.group_id) {
				// Drop this old group
				cmp::Ordering::Less => return Ok(writer),
				cmp::Ordering::Greater => state.objects.clear(),
				cmp::Ordering::Equal => {}
			}
		}

		state.objects.push(reader);
		state.epoch += 1;

		Ok(writer)
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for ObjectsWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

impl Drop for ObjectsWriter {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
	}
}

#[derive(Clone, Debug)]
pub struct ObjectsReader {
	state: Watch<ObjectsState>,
	pub track: Arc<Track>,
	epoch: usize,

	_dropped: Arc<ObjectsDropped>,
}

impl ObjectsReader {
	fn new(state: Watch<ObjectsState>, track: Arc<Track>) -> Self {
		let _dropped = Arc::new(ObjectsDropped { state: state.clone() });
		Self {
			state,
			track,
			epoch: 0,
			_dropped,
		}
	}

	pub async fn next(&mut self) -> Result<Option<ObjectReader>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.epoch < state.epoch {
					let index = state.objects.len().saturating_sub(state.epoch - self.epoch);
					self.epoch = state.epoch - state.objects.len() + index + 1;
					return Ok(Some(state.objects[index].clone()));
				}

				match &state.closed {
					Ok(()) => state.changed(),
					Err(ServeError::Done) => return Err(ServeError::Done),
					Err(err) => return Err(err.clone()),
				}
			};

			notify.await;
		}
	}

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		let state = self.state.lock();
		state
			.objects
			.iter()
			.max_by_key(|a| (a.group_id, a.object_id))
			.map(|a| (a.group_id, a.object_id))
	}
}

impl Deref for ObjectsReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

struct ObjectsDropped {
	state: Watch<ObjectsState>,
}

impl fmt::Debug for ObjectsDropped {
	fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
		Ok(())
	}
}

impl Drop for ObjectsDropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

/// Static information about the segment.
#[derive(Clone, PartialEq, Debug)]
pub struct ObjectInfo {
	pub track: Arc<Track>,

	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The priority of the stream.
	pub priority: u64,
}

impl Deref for ObjectInfo {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

impl ObjectInfo {
	pub fn produce(self) -> (ObjectWriter, ObjectReader) {
		let state = Watch::new(ObjectState::default());
		let info = Arc::new(self);

		let writer = ObjectWriter::new(state.clone(), info.clone());
		let reader = ObjectReader::new(state, info);

		(writer, reader)
	}
}

pub struct Object {
	// The sequence number of the group within the track.
	pub group_id: u64,

	// The sequence number of the object within the group.
	pub object_id: u64,

	// The priority of the stream.
	pub priority: u64,
}

struct ObjectState {
	// The data that has been received thus far.
	chunks: Vec<Bytes>,

	// Set when the writer is dropped.
	closed: Result<(), ServeError>,
}

impl ObjectState {
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl fmt::Debug for ObjectState {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("ObjectState")
			.field("chunks", &self.chunks.len())
			.field("size", &self.chunks.iter().map(|c| c.len()).sum::<usize>())
			.field("closed", &self.closed)
			.finish()
	}
}

impl Default for ObjectState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a segment and notify readers.
#[derive(Debug)]
pub struct ObjectWriter {
	// Mutable segment state.
	state: Watch<ObjectState>,

	// Immutable segment state.
	pub object: Arc<ObjectInfo>,
}

impl ObjectWriter {
	/// Create a new segment with the given info.
	fn new(state: Watch<ObjectState>, object: Arc<ObjectInfo>) -> Self {
		Self { state, object }
	}

	/// Write a new chunk of bytes.
	pub fn write(&mut self, chunk: Bytes) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.chunks.push(chunk);

		Ok(())
	}

	/// Close the segment with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Drop for ObjectWriter {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
	}
}

impl Deref for ObjectWriter {
	type Target = ObjectInfo;

	fn deref(&self) -> &Self::Target {
		&self.object
	}
}

/// Notified when a segment has new data available.
#[derive(Clone, Debug)]
pub struct ObjectReader {
	// Modify the segment state.
	state: Watch<ObjectState>,

	// Immutable segment state.
	pub object: Arc<ObjectInfo>,

	// The number of chunks that we've read.
	// NOTE: Cloned readers inherit this index, but then run in parallel.
	index: usize,

	_dropped: Arc<ObjectDropped>,
}

impl ObjectReader {
	fn new(state: Watch<ObjectState>, object: Arc<ObjectInfo>) -> Self {
		let _dropped = Arc::new(ObjectDropped::new(state.clone()));
		Self {
			state,
			object,
			index: 0,
			_dropped,
		}
	}

	/// Block until the next chunk of bytes is available.
	pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();

				if self.index < state.chunks.len() {
					let chunk = state.chunks[self.index].clone();
					self.index += 1;
					return Ok(Some(chunk));
				}

				match &state.closed {
					Err(ServeError::Done) => return Ok(None),
					Err(err) => return Err(err.clone()),
					Ok(()) => state.changed(),
				}
			};

			notify.await; // Try again when the state changes
		}
	}

	pub async fn read_all(&mut self) -> Result<Bytes, ServeError> {
		let mut chunks = Vec::new();
		while let Some(chunk) = self.read().await? {
			chunks.push(chunk);
		}

		Ok(Bytes::from(chunks.concat()))
	}
}

impl Deref for ObjectReader {
	type Target = ObjectInfo;

	fn deref(&self) -> &Self::Target {
		&self.object
	}
}

struct ObjectDropped {
	state: Watch<ObjectState>,
}

impl ObjectDropped {
	fn new(state: Watch<ObjectState>) -> Self {
		Self { state }
	}
}

impl Drop for ObjectDropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for ObjectDropped {
	fn fmt(&self, _: &mut fmt::Formatter<'_>) -> fmt::Result {
		Ok(())
	}
}
