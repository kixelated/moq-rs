use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self, GroupOrder},
};

use super::{AnnounceId, Error, ErrorCode, GroupId, Increment, PublisherStream, StreamId, SubscribeId};

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

#[derive(Clone, Debug)]
pub enum StreamKind {
	Session,
	Unknown(StreamId),
	Publisher(PublisherStream),
	Subscriber(SubscriberStream),
}

impl StreamKind {
	pub fn direction(&self) -> StreamDirection {
		match self {
			Self::SubscriberGroup(_) | Self::PublisherGroup(_) => StreamDirection::Uni,
			Self::RecvStream(id) => id.direction(),
			_ => StreamDirection::Bi,
		}
	}
}
