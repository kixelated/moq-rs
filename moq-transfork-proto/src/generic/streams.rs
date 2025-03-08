use std::collections::{hash_map, BTreeSet, HashMap};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError},
	message::{self},
};

use super::{Error, PublisherState, PublisherStream, SessionState, StreamId, SubscriberState, SubscriberStream};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum StreamDirection {
	Uni,
	Bi,
}

#[derive(Debug)]
pub enum StreamEvent {
	// A new stream is requested.
	//
	// Call `open(id)` when one is available.
	Open(StreamDirection),

	// The specified stream is now readable.
	// Call `streams().decode(id)` to read the data.
	Decodable(StreamId),

	// The specified stream is now writable.
	// Call `streams().encode(id)` to write the data.
	Encodable(StreamId),
}

pub(super) struct StreamsState {
	active: HashMap<StreamId, StreamState>,
	encodable: BTreeSet<StreamId>,
	decodable: BTreeSet<StreamId>,

	// A cached stream that has already been opened.
	cached: HashMap<StreamDirection, StreamId>,

	// A new stream is requested.
	requested: BTreeSet<StreamDirection>,
}

impl StreamsState {
	pub fn poll(&mut self) -> Option<StreamEvent> {
		if let Some(id) = self.encodable.pop_first() {
			return Some(StreamEvent::Encodable(id));
		}

		if let Some(id) = self.decodable.pop_first() {
			return Some(StreamEvent::Decodable(id));
		}

		if let Some(direction) = self.requested.pop_first() {
			return Some(StreamEvent::Open(direction));
		}

		None
	}

	pub fn open(&mut self, kind: StreamKind) -> Option<StreamId> {
		let direction = kind.direction();

		// Always request a new stream for the direction.
		self.requested.insert(direction);

		// It's a bit weird, but we cache 1 stream per direction.
		// This will avoid some back-and-forth since we can return the cached stream immediately.
		if let Some(id) = self.cached.remove(&direction) {
			self.active.insert(id, StreamState::new(kind));
			Some(id)
		} else {
			None
		}
	}

	pub fn encodable(&mut self, id: StreamId) {
		self.encodable.insert(id);
	}

	pub fn decodable(&mut self, id: StreamId) {
		self.decodable.insert(id);
	}
}

impl Default for StreamsState {
	fn default() -> Self {
		let mut requested: BTreeSet<StreamDirection> = BTreeSet::new();
		requested.insert(StreamDirection::Bi);
		requested.insert(StreamDirection::Uni);

		Self {
			active: HashMap::new(),
			encodable: BTreeSet::new(),
			decodable: BTreeSet::new(),
			cached: HashMap::new(),
			requested,
		}
	}
}

pub struct Streams<'a> {
	pub(super) state: &'a mut StreamsState,
	pub(super) session: &'a mut SessionState,
	pub(super) publisher: &'a mut PublisherState,
	pub(super) subscriber: &'a mut SubscriberState,
}

impl Streams<'_> {
	/// Returns Some if the stream can be used immediately, otherwise None.
	pub fn open(&mut self, direction: StreamDirection, id: StreamId) -> Option<Stream> {
		let entry = match self.state.active.entry(id) {
			hash_map::Entry::Occupied(_) => panic!("duplicate stream: {:?}", id),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let kind = match direction {
			StreamDirection::Uni => self.publisher.open(id).map(Into::into),
			StreamDirection::Bi => {
				if self.session.open(id) {
					Some(StreamKind::Session)
				} else {
					self.subscriber.open(id).map(Into::into)
				}
			}
		};

		if let Some(kind) = kind {
			let state = entry.insert(StreamState::new(kind));

			Some(Stream {
				id,
				state,
				session: self.session,
				publisher: self.publisher,
				subscriber: self.subscriber,
			})
		} else {
			// Save the stream for later.
			match self.state.cached.entry(direction) {
				hash_map::Entry::Occupied(_) => panic!("too many streams opened: {:?}", direction),
				hash_map::Entry::Vacant(entry) => entry.insert(id),
			};

			None
		}
	}

	pub fn get(&mut self, id: StreamId) -> Option<Stream> {
		Some(Stream {
			id,
			state: self.state.active.get_mut(&id)?,
			session: self.session,
			publisher: self.publisher,
			subscriber: self.subscriber,
		})
	}

	/// Accept a newly created stream.
	pub fn accept(&mut self, dir: StreamDirection, id: StreamId) -> Stream {
		let state = match self.state.active.entry(id) {
			hash_map::Entry::Occupied(_) => panic!("duplicate stream: {:?}", id),
			hash_map::Entry::Vacant(entry) => entry.insert(StreamState::new(StreamKind::Unknown(dir, id))),
		};

		Stream {
			id,
			state,
			session: self.session,
			publisher: self.publisher,
			subscriber: self.subscriber,
		}
	}
}

struct StreamState {
	kind: StreamKind,
	send_buffer: Bytes,
	recv_buffer: Vec<u8>,
}

impl StreamState {
	pub fn new(kind: StreamKind) -> Self {
		Self {
			kind,
			send_buffer: Bytes::new(),
			recv_buffer: Vec::new(),
		}
	}
}

pub struct Stream<'a> {
	id: StreamId,

	state: &'a mut StreamState,
	session: &'a mut SessionState,
	publisher: &'a mut PublisherState,
	subscriber: &'a mut SubscriberState,
}

impl<'a> Stream<'a> {
	pub fn id(&self) -> StreamId {
		self.id
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		// Use any data already in the buffer.
		if !self.state.send_buffer.is_empty() {
			buf.put(&mut self.state.send_buffer);
			return;
		}

		let mut overflow = BytesMut::new();
		let chain = &mut buf.chain_mut(&mut overflow);

		match self.state.kind {
			StreamKind::Session => self.session.encode(chain),
			StreamKind::Publisher(kind) => self.publisher.encode(kind, chain),
			StreamKind::Subscriber(kind) => self.subscriber.encode(kind, chain),
			StreamKind::Unknown(..) => unreachable!("unknown type"),
		};

		self.state.send_buffer = overflow.freeze();
	}

	pub fn decode(&mut self, mut buf: &[u8]) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		// Chain the Buf, so we'll decode the old data first then the new data.
		let mut buffer = std::mem::take(&mut self.state.recv_buffer);
		let chain = &mut buffer.chain(&mut buf);

		while chain.has_remaining() {
			match self.recv(chain) {
				Ok(()) => continue,
				Err(Error::Coding(DecodeError::Short)) => {
					// We need to keep the buffer for the next call.
					// Put the remainder of the buffer back.
					buffer.put(buf);
					break;
				}
				Err(err) => return Err(err),
			}
		}

		self.state.recv_buffer = buffer;

		Ok(())
	}

	// Partially decode a stream, with the remainder (on error) being put back into the buffer.
	fn recv<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		match self.state.kind {
			StreamKind::Unknown(dir, stream) => {
				self.state.kind = match dir {
					StreamDirection::Uni => match message::DataType::decode(buf)? {
						message::DataType::Group => StreamKind::Subscriber(self.subscriber.accept_group(stream, buf)?),
					},
					StreamDirection::Bi => match message::ControlType::decode(buf)? {
						message::ControlType::Session => StreamKind::Session,
						message::ControlType::Announce => {
							StreamKind::Publisher(self.publisher.accept_announce(stream, buf)?)
						}
						message::ControlType::Subscribe => {
							StreamKind::Publisher(self.publisher.accept_subscribe(stream, buf)?)
						}
						message::ControlType::Info => todo!(),
					},
				};
			}
			StreamKind::Session => self.session.decode(buf)?,
			StreamKind::Publisher(kind) => self.publisher.decode(kind, buf)?,
			StreamKind::Subscriber(kind) => self.subscriber.decode(kind, buf)?,
		}

		Ok(())
	}
}

#[derive(Clone, Debug, From)]
pub(crate) enum StreamKind {
	Session,
	Unknown(StreamDirection, StreamId),
	Publisher(PublisherStream),
	Subscriber(SubscriberStream),
}

impl StreamKind {
	pub fn direction(&self) -> StreamDirection {
		match self {
			Self::Subscriber(SubscriberStream::Group(_)) | Self::Publisher(PublisherStream::Group(_)) => {
				StreamDirection::Uni
			}
			Self::Unknown(dir, _) => *dir,
			_ => StreamDirection::Bi,
		}
	}
}
