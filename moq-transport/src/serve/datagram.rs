use std::{fmt, sync::Arc};

use crate::util::Watch;

use super::{ServeError, Track};

pub struct Datagrams {
	pub track: Arc<Track>,
}

impl Datagrams {
	pub fn produce(self) -> (DatagramsWriter, DatagramsReader) {
		let state = Watch::new(DatagramsState::default());

		let writer = DatagramsWriter::new(state.clone(), self.track.clone());
		let reader = DatagramsReader::new(state, self.track);

		(writer, reader)
	}
}

#[derive(Debug)]
struct DatagramsState {
	// The latest datagram
	latest: Option<Datagram>,

	// Increased each time datagram changes.
	epoch: u64,

	// Set when the writer or all readers are dropped.
	closed: Result<(), ServeError>,
}

impl DatagramsState {
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for DatagramsState {
	fn default() -> Self {
		Self {
			latest: None,
			epoch: 0,
			closed: Ok(()),
		}
	}
}

#[derive(Debug)]
pub struct DatagramsWriter {
	state: Watch<DatagramsState>,
	pub track: Arc<Track>,
}

impl DatagramsWriter {
	fn new(state: Watch<DatagramsState>, track: Arc<Track>) -> Self {
		Self { state, track }
	}

	pub fn write(&mut self, datagram: Datagram) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;

		state.latest = Some(datagram);
		state.epoch += 1;

		Ok(())
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}
}

impl Drop for DatagramsWriter {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
	}
}

#[derive(Debug, Clone)]
pub struct DatagramsReader {
	state: Watch<DatagramsState>,
	pub track: Arc<Track>,

	epoch: u64,
	_dropped: Arc<DatagramDropped>,
}

impl DatagramsReader {
	fn new(state: Watch<DatagramsState>, track: Arc<Track>) -> Self {
		let _dropped = Arc::new(DatagramDropped::new(state.clone()));
		Self {
			state,
			track,
			epoch: 0,
			_dropped,
		}
	}

	pub async fn read(&mut self) -> Result<Option<Datagram>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.epoch < state.epoch {
					self.epoch = state.epoch;
					return Ok(state.latest.clone());
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
			.latest
			.as_ref()
			.map(|datagram| (datagram.group_id, datagram.object_id))
	}
}

struct DatagramDropped {
	state: Watch<DatagramsState>,
}

impl DatagramDropped {
	fn new(state: Watch<DatagramsState>) -> Self {
		Self { state }
	}
}

impl Drop for DatagramDropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for DatagramDropped {
	fn fmt(&self, _f: &mut fmt::Formatter<'_>) -> fmt::Result {
		Ok(())
	}
}

/// Static information about the datagram.
#[derive(Clone)]
pub struct Datagram {
	pub group_id: u64,
	pub object_id: u64,
	pub priority: u64,
	pub payload: bytes::Bytes,
}

impl fmt::Debug for Datagram {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Datagram")
			.field("object_id", &self.object_id)
			.field("group_id", &self.group_id)
			.field("priority", &self.priority)
			.field("payload", &self.payload.len())
			.finish()
	}
}
