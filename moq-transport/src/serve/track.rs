//! A track is a collection of semi-reliable and semi-ordered streams, split into a [Writer] and [Reader] handle.
//!
//! A [Writer] creates streams with a sequence number and priority.
//! The sequest number is used to determine the order of streams, while the priority is used to determine which stream to transmit first.
//! This may seem counter-intuitive, but is designed for live streaming where the newest streams may be higher priority.
//! A cloned [Writer] can be used to create streams in parallel, but will error if a duplicate sequence number is used.
//!
//! A [Reader] may not receive all streams in order or at all.
//! These streams are meant to be transmitted over congested networks and the key to MoQ Tranport is to not block on them.
//! streams will be cached for a potentially limited duration added to the unreliable nature.
//! A cloned [Reader] will receive a copy of all new stream going forward (fanout).
//!
//! The track is closed with [ServeError::Closed] when all writers or readers are dropped.

use crate::watch::State;

use super::{
	Datagrams, DatagramsReader, DatagramsWriter, Groups, GroupsReader, GroupsWriter, Objects, ObjectsReader,
	ObjectsWriter, ServeError, Stream, StreamReader, StreamWriter,
};
use paste::paste;
use std::{ops::Deref, sync::Arc};

/// Static information about a track.
#[derive(Debug, Clone, PartialEq)]
pub struct Track {
	pub namespace: String,
	pub name: String,
}

impl Track {
	pub fn new(namespace: String, name: String) -> Self {
		Self { namespace, name }
	}

	pub fn produce(self) -> (TrackWriter, TrackReader) {
		let (writer, reader) = State::default().split();
		let info = Arc::new(self);

		let writer = TrackWriter::new(writer, info.clone());
		let reader = TrackReader::new(reader, info);

		(writer, reader)
	}
}

struct TrackState {
	mode: Option<TrackReaderMode>,
	closed: Result<(), ServeError>,
}

impl Default for TrackState {
	fn default() -> Self {
		Self {
			mode: None,
			closed: Ok(()),
		}
	}
}

/// Creates new streams for a track.
pub struct TrackWriter {
	state: State<TrackState>,
	pub info: Arc<Track>,
}

impl TrackWriter {
	/// Create a track with the given name.
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info }
	}

	pub fn stream(self, priority: u64) -> Result<StreamWriter, ServeError> {
		let (writer, reader) = Stream {
			track: self.info.clone(),
			priority,
		}
		.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn groups(self) -> Result<GroupsWriter, ServeError> {
		let (writer, reader) = Groups {
			track: self.info.clone(),
		}
		.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn objects(self) -> Result<ObjectsWriter, ServeError> {
		let (writer, reader) = Objects {
			track: self.info.clone(),
		}
		.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn datagrams(self) -> Result<DatagramsWriter, ServeError> {
		let (writer, reader) = Datagrams {
			track: self.info.clone(),
		}
		.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Cancel)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	/// Close the track with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Cancel)?;
		state.closed = Err(err);
		Ok(())
	}
}

impl Deref for TrackWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

/// Receives new streams for a track.
#[derive(Clone)]
pub struct TrackReader {
	state: State<TrackState>,
	pub info: Arc<Track>,
}

impl TrackReader {
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info }
	}

	pub async fn mode(&self) -> Result<TrackReaderMode, ServeError> {
		loop {
			{
				let state = self.state.lock();
				if let Some(mode) = &state.mode {
					return Ok(mode.clone());
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Err(ServeError::Done),
				}
			}
			.await;
		}
	}

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		// We don't even know the mode yet.
		// TODO populate from SUBSCRIBE_OK
		None
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notify) => notify,
					None => return Ok(()),
				}
			}
			.await;
		}
	}
}

impl Deref for TrackReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

macro_rules! track_readers {
    {$($name:ident,)*} => {
		paste! {
			#[derive(Clone)]
			pub enum TrackReaderMode {
				$($name([<$name Reader>])),*
			}

			$(impl From<[<$name Reader>]> for TrackReaderMode {
				fn from(reader: [<$name Reader >]) -> Self {
					Self::$name(reader)
				}
			})*

			impl TrackReaderMode {
				pub fn latest(&self) -> Option<(u64, u64)> {
					match self {
						$(Self::$name(reader) => reader.latest(),)*
					}
				}
			}
		}
	}
}

track_readers!(Stream, Groups, Objects, Datagrams,);

macro_rules! track_writers {
    {$($name:ident,)*} => {
		paste! {
			pub enum TrackWriterMode {
				$($name([<$name Writer>])),*
			}

			$(impl From<[<$name Writer>]> for TrackWriterMode {
				fn from(writer: [<$name Writer>]) -> Self {
					Self::$name(writer)
				}
			})*

			impl TrackWriterMode {
				pub fn close(self, err: ServeError) -> Result<(), ServeError>{
					match self {
						$(Self::$name(writer) => writer.close(err),)*
					}
				}
			}
		}
	}
}

track_writers!(Track, Stream, Groups, Objects, Datagrams,);
