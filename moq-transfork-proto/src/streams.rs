use std::{
	collections::{hash_map, BTreeSet, HashMap},
	fmt,
};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError},
	message::{self},
};

use super::{Error, PublisherStream, StreamId, SubscriberStream};

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

	// The specified stream is now writable.
	// Call `get(id).encode()` to write the data.
	Encode(StreamId),
}

pub struct Streams {
	active: HashMap<StreamId, Stream>,
}

impl Streams {
	// Request that a stream is opened.
	/*
	pub(crate) fn open(&mut self, kind: StreamKind) {
		let direction = kind.direction();

		// Request a new stream for the direction.
		self.requested.insert(direction);
	}

	pub(crate) fn encodable(&mut self, id: StreamId) {
		self.encodable.insert(id);
	}
	*/

	/// Call when a new stream is opened.
	pub fn opened(&mut self, direction: StreamDirection, id: StreamId) -> &mut Stream {
		let entry = match self.active.entry(id) {
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

		let kind = kind.expect("open called when not needed");
		let state = entry.insert(Stream::new(id, kind));

		state
	}

	pub fn get(&mut self, id: StreamId) -> Option<&mut Stream> {
		self.active.get_mut(&id)
	}

	pub fn accepted(&mut self, dir: StreamDirection, id: StreamId) -> &mut Stream {
		let state = match self.active.entry(id) {
			hash_map::Entry::Occupied(_) => panic!("duplicate stream: {:?}", id),
			hash_map::Entry::Vacant(entry) => entry.insert(Stream::new(id, StreamKind::Unknown(dir, id))),
		};

		state
	}
}

pub struct Stream {
	id: StreamId,
	send_buffer: Bytes,
	recv_buffer: Vec<u8>,
}

impl Stream {
	pub(crate) fn new(id: StreamId, kind: StreamKind) -> Self {
		Self {
			id,
			send_buffer: Bytes::new(),
			recv_buffer: Vec::new(),
		}
	}

	pub fn id(&self) -> StreamId {
		self.id
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		// Use any data already in the buffer.
		if !self.send_buffer.is_empty() {
			buf.put(&mut self.send_buffer);
			return;
		}

		let mut overflow = BytesMut::new();
		let chain = &mut buf.chain_mut(&mut overflow);

		match self.kind {
			StreamKind::Session => todo!(),
			StreamKind::Publisher(kind) => todo!(),
			StreamKind::Subscriber(kind) => todo!(),
			StreamKind::Unknown(..) => unreachable!("unknown type"),
		};

		self.send_buffer = overflow.freeze();
	}

	pub fn decode(&mut self, mut buf: &[u8]) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		// Chain the Buf, so we'll decode the old data first then the new data.
		let mut buffer = std::mem::take(&mut self.recv_buffer);
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

		self.recv_buffer = buffer;

		Ok(())
	}

	// Partially decode a stream, with the remainder (on error) being put back into the buffer.
	fn recv<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		match self.kind {
			StreamKind::Unknown(dir, stream) => {
				self.kind = match dir {
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
