use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self},
};

use super::{
	Error, PublisherState, PublisherStream, SessionState,
	StreamId, SubscriberState, SubscriberStream,
};

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, PartialOrd, Ord)]
pub enum StreamDirection {
	Uni,
	Bi,
}

#[derive(Debug)]
pub enum StreamEvent {
	// A new stream is requested.
	//
	// Use `kind.direction()` to determine the direction of the stream.
	// Call `open(id, kind)` when one is available.
	Open(StreamKind),

	// The specified stream is now readable.
	// Call `streams().decode(id)` to read the data.
	Readable(StreamId),

	// The specified stream is now writable.
	// Call `streams().encode(id)` to write the data.
	Writable(StreamId),
}

#[derive(Default)]
pub(super) struct StreamsState {
	active: HashMap<StreamId, Stream>,

	// A list of streams that we want created.
	create: VecDeque<StreamKind>,

	encodable: BTreeSet<StreamId>,
	decodable: BTreeSet<StreamId>,
}

impl StreamsState {
	pub fn poll(&mut self) -> Option<StreamEvent> {
		if let Some(id) = self.encodable.pop_first() {
			return Some(StreamEvent::Writable(id));
		}

		if let Some(id) = self.decodable.pop_first() {
			return Some(StreamEvent::Readable(id));
		}

		if let Some(kind) = self.create.pop_front() {
			return Some(StreamEvent::Open(kind));
		}

		None
	}

	pub fn get(&self, id: StreamId) -> Option<&Stream> {
		self.active.get(&id)
	}

	pub fn get_mut(&mut self, id: StreamId) -> Option<&mut Stream> {
		self.active.get_mut(&id)
	}

	pub fn get_or_create(&mut self, id: StreamId) -> &mut Stream {
		self.active
			.entry(id)
			.or_insert_with(|| Stream::new(StreamKind::Unknown(id)))
	}

	pub fn create(&mut self, kind: StreamKind) {
		self.create.push_back(kind);
	}

	pub fn encodable(&mut self, id: StreamId) {
		self.encodable.insert(id);
	}

	pub fn decodable(&mut self, id: StreamId) {
		self.decodable.insert(id);
	}
}

pub struct Streams<'a> {
	pub(super) state: &'a mut StreamsState,
	pub(super) session: &'a mut SessionState,
	pub(super) publisher: &'a mut PublisherState,
	pub(super) subscriber: &'a mut SubscriberState,
}

impl Streams<'_> {
	pub fn encode<B: BufMut>(&mut self, id: StreamId, buf: &mut B) {
		let stream = self.state.get_mut(id).unwrap();

		// Use any data already in the buffer.
		if !stream.send_buffer.is_empty() {
			buf.put(&mut stream.send_buffer);
			return;
		}

		let mut overflow = BytesMut::new();
		let chain = &mut buf.chain_mut(&mut overflow);

		match stream.kind {
			StreamKind::Session => self.session.encode(chain),
			StreamKind::Publisher(kind) => self.publisher.encode(kind, chain),
			StreamKind::Subscriber(kind) => self.subscriber.encode(kind, chain),
			StreamKind::Unknown(_) => unreachable!("unknown type"),
		};

		stream.send_buffer = overflow.freeze();
	}

	pub fn decode(&mut self, id: StreamId, mut buf: &[u8]) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		let stream = self.state.get_or_create(id);

		// Chain the Buf, so we'll decode the old data first then the new data.
		let chain = &mut stream.recv_buffer.chain(&mut buf);

		while chain.has_remaining() {
			match Self::recv(
				&mut stream.kind,
				chain,
				self.session,
				self.publisher,
				self.subscriber,
			) {
				Ok(()) => continue,
				Err(Error::Coding(DecodeError::Short)) => {
					// We need to keep the buffer for the next call.
					// Put the remainder of the buffer back.
					stream.recv_buffer.put(buf);
					return Ok(());
				}
				Err(err) => return Err(err),
			}
		}

		Ok(())
	}

	pub fn open(&mut self, id: StreamId, kind: StreamKind) {
		match kind {
			StreamKind::Subscriber(kind) => self.subscriber.open(id, kind),
			StreamKind::Publisher(kind) => self.publisher.open(id, kind),
			StreamKind::Session => self.session.open(id),
			_ => unreachable!(),
		};

		self.state.encodable(id);
	}

	// Partially decode a stream, with the remainder (on error) being put back into the buffer.
	// This doesn't take self because StreamsState is partially borrowed.
	fn recv<B: Buf>(
		kind: &mut StreamKind,
		buf: &mut B,
		session: &mut SessionState,
		publisher: &mut PublisherState,
		subscriber: &mut SubscriberState,
	) -> Result<(), Error> {
		match *kind {
			StreamKind::Unknown(stream) => {
				*kind = if stream.is_uni() {
					match message::DataType::decode(buf)? {
						message::DataType::Group => StreamKind::Subscriber(subscriber.accept_group(stream, buf)?),
					}
				} else {
					match message::ControlType::decode(buf)? {
						message::ControlType::Session => {
							session.accept(stream, buf)?;
							StreamKind::Session
						}
						message::ControlType::Announce => {
							StreamKind::Publisher(publisher.accept_announce(stream, buf)?)
						}
						message::ControlType::Subscribe => {
							StreamKind::Publisher(publisher.accept_subscribe(stream, buf)?)
						}
						message::ControlType::Info => todo!(),
					}
				}
			}
			StreamKind::Session => session.decode(buf)?,
			StreamKind::Publisher(kind) => publisher.decode(kind, buf)?,
			StreamKind::Subscriber(kind) => subscriber.decode(kind, buf)?,
		}

		Ok(())
	}
}

struct Stream {
	pub kind: StreamKind,
	pub send_buffer: Bytes,
	pub recv_buffer: Vec<u8>,
}

impl Stream {
	pub fn new(kind: StreamKind) -> Self {
		Self {
			kind,
			send_buffer: Bytes::new(),
			recv_buffer: Vec::new(),
		}
	}
}

#[derive(Clone, Debug, From)]
pub enum StreamKind {
	Session,
	Unknown(StreamId),
	Publisher(PublisherStream),
	Subscriber(SubscriberStream),
}

impl StreamKind {
	pub fn direction(&self) -> StreamDirection {
		match self {
			Self::Subscriber(SubscriberStream::Group(_)) | Self::Publisher(PublisherStream::Group(_)) => {
				StreamDirection::Uni
			}
			Self::Unknown(id) => id.direction(),
			_ => StreamDirection::Bi,
		}
	}
}
