use std::collections::{HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, Stream, StreamId,
};

pub enum SubscriberAnnounceEvent {}

#[derive(Default)]
pub struct SubscriberAnnounces {
	lookup: HashMap<StreamId, SubscriberAnnounce>,
}

impl SubscriberAnnounces {
	pub fn create(&mut self, stream: Stream, request: message::AnnouncePlease) -> &mut SubscriberAnnounce {
		let id = stream.id();
		let announce = SubscriberAnnounce::new(stream, request);
		self.lookup.entry(id).or_insert(announce)
	}

	pub fn get(&mut self, stream: StreamId) -> Option<&mut SubscriberAnnounce> {
		self.lookup.get_mut(&stream)
	}
}

pub struct SubscriberAnnounce {
	request: Option<message::AnnouncePlease>,
	stream: Stream,
}

impl SubscriberAnnounce {
	fn new(stream: Stream, request: message::AnnouncePlease) -> Self {
		Self {
			request: Some(request),
			stream,
		}
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(request) = self.request.take() {
			self.stream.encode(buf, request);
		}
	}

	/// Decode the next frame from the stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<Option<message::Announce>, Error> {
		self.stream.try_decode(buf)
	}
}
