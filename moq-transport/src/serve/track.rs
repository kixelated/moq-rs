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

use crate::util::Watch;

use super::{
	Datagrams, DatagramsReader, DatagramsWriter, Groups, GroupsReader, GroupsWriter, Objects, ObjectsReader,
	ObjectsWriter, ServeError, Stream, StreamReader, StreamWriter,
};
use paste::paste;
use std::{fmt, ops::Deref, sync::Arc};

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
		let state = Watch::new(TrackState::default());
		let info = Arc::new(self);

		let writer = TrackWriter::new(state.clone(), info.clone());
		let reader = TrackReader::new(state, info);

		(writer, reader)
	}
}

#[derive(Debug)]
struct TrackState {
	mode: Option<TrackReaderMode>,
	closed: Result<(), ServeError>,
}

impl TrackState {
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err);
		Ok(())
	}
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
	state: Watch<TrackState>,
	pub track: Arc<Track>,
}

impl TrackWriter {
	/// Create a track with the given name.
	fn new(state: Watch<TrackState>, track: Arc<Track>) -> Self {
		Self { state, track }
	}

	pub fn stream(self, priority: u64) -> StreamWriter {
		let streams = Stream {
			track: self.track.clone(),
			priority,
		};
		let (writer, reader) = streams.produce();

		self.state.lock_mut().mode = Some(reader.into());
		writer
	}

	pub fn groups(self) -> GroupsWriter {
		let groups = Groups {
			track: self.track.clone(),
		};
		let (writer, reader) = groups.produce();

		self.state.lock_mut().mode = Some(reader.into());
		writer
	}

	pub fn objects(self) -> ObjectsWriter {
		let objects = Objects {
			track: self.track.clone(),
		};
		let (writer, reader) = objects.produce();

		self.state.lock_mut().mode = Some(reader.into());
		writer
	}

	pub fn datagrams(self) -> DatagramsWriter {
		let datagrams = Datagrams {
			track: self.track.clone(),
		};
		let (writer, reader) = datagrams.produce();

		self.state.lock_mut().mode = Some(reader.into());
		writer
	}

	/// Close the track with an error.
	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
	}

	pub fn closed(&self) -> Result<(), ServeError> {
		self.state.lock().closed.clone()
	}
}

impl Deref for TrackWriter {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
	}
}

impl Drop for TrackWriter {
	fn drop(&mut self) {
		let state = self.state.lock();
		if state.mode.is_none() {
			state.into_mut().close(ServeError::Done).ok();
		}
	}
}

/// Receives new streams for a track.
#[derive(Clone, Debug)]
pub struct TrackReader {
	state: Watch<TrackState>,
	pub track: Arc<Track>,
	_dropped: Arc<TrackDropped>,
}

impl TrackReader {
	fn new(state: Watch<TrackState>, track: Arc<Track>) -> Self {
		let _dropped = Arc::new(TrackDropped::new(state.clone()));
		Self { state, track, _dropped }
	}

	pub async fn mode(self) -> Result<TrackReaderMode, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if let Some(mode) = &state.mode {
					return Ok(mode.clone());
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
		// We don't even know the mode yet.
		// TODO populate from SUBSCRIBE_OK
		None
	}
}

impl Deref for TrackReader {
	type Target = Track;

	fn deref(&self) -> &Self::Target {
		&self.track
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
				pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
					match self {
						$(Self::$name(writer) => writer.close(err),)*
					}
				}
			}
		}
	}
}

track_writers!(Track, Stream, Groups, Objects, Datagrams,);

struct TrackDropped {
	state: Watch<TrackState>,
}

impl TrackDropped {
	fn new(state: Watch<TrackState>) -> Self {
		Self { state }
	}
}

impl Drop for TrackDropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(ServeError::Done).ok();
	}
}

impl fmt::Debug for TrackDropped {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("TrackDropped").finish()
	}
}
