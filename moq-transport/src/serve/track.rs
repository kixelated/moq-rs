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

use crate::util::{State, StateWeak};

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
	pub fn new(namespace: &str, name: &str) -> Self {
		Self {
			namespace: namespace.to_string(),
			name: name.to_string(),
		}
	}

	pub fn produce(self) -> (TrackWriter, TrackReader) {
		let (writer, reader) = State::default();
		let info = Arc::new(self);

		let writer = TrackWriter::new(writer, info.clone());
		let reader = TrackReader::new(reader, info);

		(writer, reader)
	}
}

#[derive(Debug)]
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
#[derive(Debug)]
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
		let streams = Stream {
			track: self.info.clone(),
			priority,
		};
		let (writer, reader) = streams.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn groups(self) -> Result<GroupsWriter, ServeError> {
		let groups = Groups {
			track: self.info.clone(),
		};
		let (writer, reader) = groups.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn objects(self) -> Result<ObjectsWriter, ServeError> {
		let objects = Objects {
			track: self.info.clone(),
		};
		let (writer, reader) = objects.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	pub fn datagrams(self) -> Result<DatagramsWriter, ServeError> {
		let datagrams = Datagrams {
			track: self.info.clone(),
		};
		let (writer, reader) = datagrams.produce();

		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.mode = Some(reader.into());
		Ok(writer)
	}

	/// Close the track with an error.
	pub fn close(self, err: ServeError) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
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
#[derive(Clone, Debug)]
pub struct TrackReader {
	state: State<TrackState>,
	pub info: Arc<Track>,
}

impl TrackReader {
	fn new(state: State<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info }
	}

	pub async fn mode(self) -> Result<TrackReaderMode, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if let Some(mode) = &state.mode {
					return Ok(mode.clone());
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notify) => notify,
					None => return Err(ServeError::Done),
				}
			};

			notify.await;
		}
	}

	// Returns the largest group/sequence
	pub fn latest(&self) -> Option<(u64, u64)> {
		// We don't even know the mode yet.
		// TODO populate from SUBSCRIBE_OK
		None
	}

	pub fn weak(&self) -> TrackReaderWeak {
		TrackReaderWeak::new(self.state.downgrade(), self.info.clone())
	}
}

impl Deref for TrackReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone)]
pub struct TrackReaderWeak {
	state: StateWeak<TrackState>,
	pub info: Arc<Track>,
}

impl TrackReaderWeak {
	fn new(state: StateWeak<TrackState>, info: Arc<Track>) -> Self {
		Self { state, info }
	}

	pub fn upgrade(&self) -> Option<TrackReader> {
		let state = self.state.upgrade()?;
		Some(TrackReader::new(state, self.info.clone()))
	}
}

macro_rules! track_readers {
    {$($name:ident,)*} => {
		paste! {
			#[derive(Debug, Clone)]
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
			#[derive(Debug)]
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
