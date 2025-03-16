use std::collections::{HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, StreamId,
};

pub enum SubscriberAnnounceEvent {}

#[derive(Default)]
pub struct SubscriberAnnounces {
	lookup: HashMap<StreamId, SubscriberAnnounce>,
}

impl SubscriberAnnounces {
	pub fn create(&mut self, stream: StreamId, request: message::AnnouncePlease) -> &mut SubscriberAnnounce {
		let announce = SubscriberAnnounce::new(stream, request);
		self.lookup.entry(stream).or_insert(announce)
	}

	pub fn get(&mut self, stream: StreamId) -> Option<&mut SubscriberAnnounce> {
		self.lookup.get_mut(&stream)
	}
}

struct SubscriberAnnounce {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
	stream: StreamId,
}

impl SubscriberAnnounce {
	fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			request: Some(request),
			events: VecDeque::new(),
			stream,
		}
	}

	pub fn stream(&self) -> StreamId {
		self.stream
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let dropped = message::Announce::decode(buf)?;
		self.events.push_back(dropped);

		Ok(())
	}

	/// Returns the next announcement with pending data.
	pub fn poll(&mut self) -> Option<message::Announce> {
		self.events.pop_front()
	}
}
