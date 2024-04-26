use bytes::Bytes;
use std::{ops::Deref, sync::Arc};

use crate::watch::State;

use super::{ServeError, Track};

#[derive(Debug, PartialEq, Clone)]
pub struct Stream {
	pub track: Arc<Track>,
	pub priority: u64,
}

impl Stream {
	pub fn produce(self) -> (StreamWriter, StreamReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = StreamWriter::new(writer, info.clone());
		let reader = StreamReader::new(reader, info);

		(writer, reader)
	}
}

impl Deref for Stream {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

struct StreamState {
	// The latest group.
	latest: Option<StreamGroupReader>,

	// Updated each time objects changes.
	epoch: usize,

	// Set when the writer is dropped.
	closed: Result<(), ServeError>,
}

impl Default for StreamState {
	fn default() -> Self {
		Self {
			latest: None,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

/// Used to write data to a stream and notify readers.
///
/// This is Clone as a work-around, but be very careful because it's meant to be sequential.
#[derive(Clone)]
pub struct StreamWriter {
	// Mutable stream state.
	state: State<StreamState>,

	// Immutable stream state.
	pub info: Arc<Stream>,
}

impl StreamWriter {
	fn new(state: State<StreamState>, info: Arc<Stream>) -> Self {
		Self { state, info }
	}

	pub fn create(&mut self, group_id: u64) -> Result<StreamGroupWriter, ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;

		if let Some(latest) = &state.latest {
			if latest.group_id > group_id {
				return Err(ServeError::Duplicate);
			}
		}

		let group = Arc::new(StreamGroup {
			stream: self.info.clone(),
			group_id,
		});

		let (writer, reader) = State::default().split();

		let reader = StreamGroupReader::new(reader, group.clone());
		let writer = StreamGroupWriter::new(writer, group);

		state.latest = Some(reader);
		state.epoch += 1;

		Ok(writer)
	}

	pub fn append(&mut self) -> Result<StreamGroupWriter, ServeError> {
		let next = self
			.state
			.lock()
			.latest
			.as_ref()
			.map(|g| g.group_id + 1)
			.unwrap_or_default();
		self.create(next)
	}

	/// Close the stream with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Deref for StreamWriter {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a stream has new data available.
#[derive(Clone)]
pub struct StreamReader {
	// Modify the stream state.
	state: State<StreamState>,

	// Immutable stream state.
	pub info: Arc<Stream>,

	// The number of chunks that we've read.
	// NOTE: Cloned readers inherit this index, but then run in parallel.
	epoch: usize,
}

impl StreamReader {
	fn new(state: State<StreamState>, info: Arc<Stream>) -> Self {
		Self { state, info, epoch: 0 }
	}

	/// Block until the next group is available.
	pub async fn next(&mut self) -> Result<Option<StreamGroupReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();
				if self.epoch != state.epoch {
					self.epoch = state.epoch;
					let latest = state.latest.clone().unwrap();
					return Ok(Some(latest));
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

impl Deref for StreamReader {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone, PartialEq, Debug)]
pub struct StreamGroup {
	pub stream: Arc<Stream>,
	pub group_id: u64,
}

impl Deref for StreamGroup {
	type Target = Stream;

	fn deref(&self) -> &Self::Target {
		&self.stream
	}
}

struct StreamGroupState {
	// The objects that have been received thus far.
	objects: Vec<StreamObjectReader>,
	closed: Result<(), ServeError>,
}

impl Default for StreamGroupState {
	fn default() -> Self {
		Self {
			objects: Vec::new(),
			closed: Ok(()),
		}
	}
}

pub struct StreamGroupWriter {
	state: State<StreamGroupState>,
	pub info: Arc<StreamGroup>,
	next: u64,
}

impl StreamGroupWriter {
	fn new(state: State<StreamGroupState>, info: Arc<StreamGroup>) -> Self {
		Self { state, info, next: 0 }
	}

	/// Add a new object to the group.
	pub fn write(&mut self, payload: Bytes) -> Result<(), ServeError> {
		let mut writer = self.create(payload.len())?;
		writer.write(payload)?;
		Ok(())
	}

	pub fn create(&mut self, size: usize) -> Result<StreamObjectWriter, ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;

		let (writer, reader) = StreamObject {
			group: self.info.clone(),
			object_id: self.next,
			size,
		}
		.produce();

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
}

impl Deref for StreamGroupWriter {
	type Target = StreamGroup;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct StreamGroupReader {
	pub info: Arc<StreamGroup>,
	state: State<StreamGroupState>,
	index: usize,
}

impl StreamGroupReader {
	fn new(state: State<StreamGroupState>, info: Arc<StreamGroup>) -> Self {
		Self { state, info, index: 0 }
	}

	pub async fn read_next(&mut self) -> Result<Option<Bytes>, ServeError> {
		if let Some(mut reader) = self.next().await? {
			Ok(Some(reader.read_all().await?))
		} else {
			Ok(None)
		}
	}

	pub async fn next(&mut self) -> Result<Option<StreamObjectReader>, ServeError> {
		loop {
			{
				let state = self.state.lock();
				if self.index < state.objects.len() {
					self.index += 1;
					return Ok(Some(state.objects[self.index].clone()));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Ok(None),
				}
			}
			.await;
		}
	}

	pub fn latest(&self) -> u64 {
		let state = self.state.lock();
		state.objects.last().map(|o| o.object_id).unwrap_or_default()
	}
}

impl Deref for StreamGroupReader {
	type Target = StreamGroup;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// A subset of Object, since we use the group's info.
#[derive(Clone, PartialEq, Debug)]
pub struct StreamObject {
	// The group this belongs to.
	pub group: Arc<StreamGroup>,

	pub object_id: u64,

	// The size of the object.
	pub size: usize,
}

impl StreamObject {
	pub fn produce(self) -> (StreamObjectWriter, StreamObjectReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = StreamObjectWriter::new(writer, info.clone());
		let reader = StreamObjectReader::new(reader, info);

		(writer, reader)
	}
}

impl Deref for StreamObject {
	type Target = StreamGroup;

	fn deref(&self) -> &Self::Target {
		&self.group
	}
}

struct StreamObjectState {
	// The data that has been received thus far.
	chunks: Vec<Bytes>,

	closed: Result<(), ServeError>,
}

impl Default for StreamObjectState {
	fn default() -> Self {
		Self {
			chunks: Vec::new(),
			closed: Ok(()),
		}
	}
}

/// Used to write data to a segment and notify readers.
pub struct StreamObjectWriter {
	// Mutable segment state.
	state: State<StreamObjectState>,

	// Immutable segment state.
	pub info: Arc<StreamObject>,

	// The amount of promised data that has yet to be written.
	remain: usize,
}

impl StreamObjectWriter {
	/// Create a new segment with the given info.
	fn new(state: State<StreamObjectState>, info: Arc<StreamObject>) -> Self {
		Self {
			state,
			remain: info.size,
			info,
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

	/// Close the stream with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);

		Ok(())
	}
}

impl Drop for StreamObjectWriter {
	// Make sure we fully write the segment, otherwise close it with an error.
	fn drop(&mut self) {
		if self.remain == 0 {
			return;
		}

		let state = self.state.lock();
		if state.closed.is_err() {
			return;
		}

		if let Some(mut state) = state.into_mut() {
			state.closed = Err(ServeError::Size);
		}
	}
}

impl Deref for StreamObjectWriter {
	type Target = StreamObject;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Notified when a segment has new data available.
#[derive(Clone)]
pub struct StreamObjectReader {
	// Modify the segment state.
	state: State<StreamObjectState>,

	// Immutable segment state.
	pub info: Arc<StreamObject>,

	// The number of chunks that we've read.
	// NOTE: Cloned readers inherit this index, but then run in parallel.
	index: usize,
}

impl StreamObjectReader {
	fn new(state: State<StreamObjectState>, info: Arc<StreamObject>) -> Self {
		Self { state, info, index: 0 }
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
					None => return Ok(None),
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

impl Deref for StreamObjectReader {
	type Target = StreamObject;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
