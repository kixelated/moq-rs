//! A stream is a stream of objects with a header, split into a [Writer] and [Reader] handle.
//!
//! A [Writer] writes an ordered stream of objects.
//! Each object can have a sequence number, allowing the reader to detect gaps objects.
//!
//! A [Reader] reads an ordered stream of objects.
//! The reader can be cloned, in which case each reader receives a copy of each object. (fanout)
//!
//! The stream is closed with [ServeError::Closed] when all writers or readers are dropped.
use bytes::Bytes;
use std::{cmp, ops::Deref, sync::Arc};

use crate::watch::State;

use super::{ServeError, Track};

pub struct Groups {
	pub track: Arc<Track>,
}

impl Groups {
	pub fn produce(self) -> (GroupsWriter, GroupsReader) {
		let (writer, reader) = State::default().split();

		let writer = GroupsWriter::new(writer, self.track.clone());
		let reader = GroupsReader::new(reader, self.track);

		(writer, reader)
	}
}

impl Deref for Groups {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

// State shared between the writer and reader.
struct GroupsState {
	latest: Option<GroupReader>,
	epoch: u64, // Updated each time latest changes
	closed: Result<(), ServeError>,
}

impl Default for GroupsState {
	fn default() -> Self {
		Self {
			latest: None,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

pub struct GroupsWriter {
	pub info: Arc<Track>,
	state: State<GroupsState>,
	next: u64, // Not in the state to avoid a lock
}

impl GroupsWriter {
	fn new(state: State<GroupsState>, track: Arc<Track>) -> Self {
		Self {
			info: track,
			state,
			next: 0,
		}
	}

	// Helper to increment the group by one.
	pub fn append(&mut self, priority: u64) -> Result<GroupWriter, ServeError> {
		self.create(Group {
			group_id: self.next,
			priority,
		})
	}

	pub fn create(&mut self, group: Group) -> Result<GroupWriter, ServeError> {
		let group = GroupInfo {
			track: self.info.clone(),
			group_id: group.group_id,
			priority: group.priority,
		};
		let (writer, reader) = group.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;

		if let Some(latest) = &state.latest {
			match writer.group_id.cmp(&latest.group_id) {
				cmp::Ordering::Less => return Ok(writer), // dropped immediately, lul
				cmp::Ordering::Equal => return Err(ServeError::Duplicate),
				cmp::Ordering::Greater => state.latest = Some(reader),
			}
		} else {
			state.latest = Some(reader);
		}

		self.next = state.latest.as_ref().unwrap().group_id + 1;
		state.epoch += 1;

		Ok(writer)
	}

	/// Close the segment with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Deref for GroupsWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct GroupsReader {
	pub info: Arc<Track>,
	state: State<GroupsState>,
	epoch: u64,
}

impl GroupsReader {
	fn new(state: State<GroupsState>, track: Arc<Track>) -> Self {
		Self {
			info: track,
			state,
			epoch: 0,
		}
	}

	pub async fn next(&mut self) -> Result<Option<GroupReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();

				if self.epoch != state.epoch {
					self.epoch = state.epoch;
					return Ok(state.latest.clone());
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			}
			.await; // Try again when the state changes
		}
	}

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		let state = self.state.lock();
		state.latest.as_ref().map(|group| (group.group_id, group.latest()))
	}
}

impl Deref for GroupsReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Parameters that can be specified by the user
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
	// The sequence number of the group within the track.
	// NOTE: These may be received out of order or with gaps.
	pub group_id: u64,

	// The priority of the group within the track.
	pub priority: u64,
}

/// Static information about the group
#[derive(Debug, Clone, PartialEq)]
pub struct GroupInfo {
	pub track: Arc<Track>,

	// The sequence number of the group within the track.
	// NOTE: These may be received out of order or with gaps.
	pub group_id: u64,

	// The priority of the group within the track.
	pub priority: u64,
}

impl GroupInfo {
	pub fn produce(self) -> (GroupWriter, GroupReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = GroupWriter::new(writer, info.clone());
		let reader = GroupReader::new(reader, info);

		(writer, reader)
	}
}

impl Deref for GroupInfo {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

struct GroupState {
	// The data that has been received thus far.
	objects: Vec<GroupObjectReader>,

	// Set when the writer or all readers are dropped.
	closed: Result<(), ServeError>,
}

impl Default for GroupState {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify readers.
pub struct GroupWriter {
	// Mutable stream state.
	state: State<GroupState>,

	// Immutable stream state.
	pub info: Arc<GroupInfo>,

	// The next object sequence number to use.
	next: u64,
}

impl GroupWriter {
	fn new(state: State<GroupState>, group: Arc<GroupInfo>) -> Self {
		Self {
			state,
			info: group,
			next: 0,
		}
	}

	/// Create the next object ID with the given payload.
	pub fn write(&mut self, payload: bytes::Bytes) -> Result<(), ServeError> {
		let mut object = self.create(payload.len())?;
		object.write(payload)?;
		Ok(())
	}

	/// Write an object over multiple writes.
	///
	/// BAD STUFF will happen if the size is wrong; this is an advanced feature.
	pub fn create(&mut self, size: usize) -> Result<GroupObjectWriter, ServeError> {
		let (writer, reader) = GroupObject {
			group: self.info.clone(),
			object_id: self.next,
			size,
		}
		.produce();

		self.next += 1;

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.objects.push(reader);

		Ok(writer)
	}

	/// Close the stream with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);
		Ok(())
	}

	pub fn len(&self) -> usize {
		self.state.lock().objects.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl Deref for GroupWriter {
	type Target = GroupInfo;

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
	pub info: Arc<GroupInfo>,

	// The number of chunks that we've read.
	// NOTE: Cloned readers inherit this index, but then run in parallel.
	index: usize,
}

impl GroupReader {
	fn new(state: State<GroupState>, group: Arc<GroupInfo>) -> Self {
		Self {
			state,
			info: group,
			index: 0,
		}
	}

	pub fn latest(&self) -> u64 {
		let state = self.state.lock();
		state.objects.last().map(|o| o.object_id).unwrap_or_default()
	}

	pub async fn read_next(&mut self) -> Result<Option<Bytes>, ServeError> {
		let object = self.next().await?;
		match object {
			Some(mut object) => Ok(Some(object.read_all().await?)),
			None => Ok(None),
		}
	}

	pub async fn next(&mut self) -> Result<Option<GroupObjectReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();

				if self.index < state.objects.len() {
					let object = state.objects[self.index].clone();
					self.index += 1;
					return Ok(Some(object));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			}
			.await; // Try again when the state changes
		}
	}

	pub fn pos(&self) -> usize {
		self.index
	}

	pub fn len(&self) -> usize {
		self.state.lock().objects.len()
	}

	pub fn is_empty(&self) -> bool {
		self.len() == 0
	}
}

impl Deref for GroupReader {
	type Target = GroupInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// A subset of Object, since we use the group's info.
#[derive(Clone, PartialEq, Debug)]
pub struct GroupObject {
	pub group: Arc<GroupInfo>,

	pub object_id: u64,

	// The size of the object.
	pub size: usize,
}

impl GroupObject {
	pub fn produce(self) -> (GroupObjectWriter, GroupObjectReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = GroupObjectWriter::new(writer, info.clone());
		let reader = GroupObjectReader::new(reader, info);

		(writer, reader)
	}
}

impl Deref for GroupObject {
	type Target = GroupInfo;

	fn deref(&self) -> &Self::Target {
		&self.group
	}
}

struct GroupObjectState {
	// The data that has been received thus far.
	chunks: Vec<Bytes>,

	// Set when the writer is dropped.
	closed: Result<(), ServeError>,
}

impl Default for GroupObjectState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a segment and notify readers.
pub struct GroupObjectWriter {
	// Mutable segment state.
	state: State<GroupObjectState>,

	// Immutable segment state.
	pub info: Arc<GroupObject>,

	// The amount of promised data that has yet to be written.
	remain: usize,
}

impl GroupObjectWriter {
	/// Create a new segment with the given info.
	fn new(state: State<GroupObjectState>, object: Arc<GroupObject>) -> Self {
		Self {
			state,
			remain: object.size,
			info: object,
		}
	}

	/// Write a new chunk of bytes.
	pub fn write(&mut self, chunk: Bytes) -> Result<(), ServeError> {
		if chunk.len() > self.remain {
			return Err(ServeError::Size);
		}
		self.remain -= chunk.len();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.chunks.push(chunk);

		Ok(())
	}

	/// Close the segment with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		if self.remain != 0 {
			return Err(ServeError::Size);
		}

		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Drop for GroupObjectWriter {
	fn drop(&mut self) {
		if self.remain == 0 {
			return;
		}

		if let Some(mut state) = self.state.lock_mut() {
			state.closed = Err(ServeError::Size);
		}
	}
}

impl Deref for GroupObjectWriter {
	type Target = GroupObject;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a segment has new data available.
#[derive(Clone)]
pub struct GroupObjectReader {
	// Modify the segment state.
	state: State<GroupObjectState>,

	// Immutable segment state.
	pub info: Arc<GroupObject>,

	// The number of chunks that we've read.
	// NOTE: Cloned readers inherit this index, but then run in parallel.
	index: usize,
}

impl GroupObjectReader {
	fn new(state: State<GroupObjectState>, object: Arc<GroupObject>) -> Self {
		Self {
			state,
			info: object,
			index: 0,
		}
	}

	/// Block until the next chunk of bytes is available.
	pub async fn read(&mut self) -> Result<Option<Bytes>, ServeError> {
		loop {
			{
				let state = self.state.lock();

				if self.index < state.chunks.len() {
					let chunk = state.chunks[self.index].clone();
					self.index += 1;
					return Ok(Some(chunk));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None), // No more changes will come
				}
			}
			.await; // Try again when the state changes
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

impl Deref for GroupObjectReader {
	type Target = GroupObject;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
