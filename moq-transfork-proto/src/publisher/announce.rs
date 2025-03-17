use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message, AnnounceId, Error, StreamId,
};

#[derive(Default)]
pub struct PublisherAnnounces {
	lookup: HashMap<StreamId, PublisherAnnounce>,
	ready: BTreeSet<StreamId>,
}

impl PublisherAnnounces {
	pub fn accept(&mut self) -> Option<&mut PublisherAnnounce> {
		let id = self.ready.pop_first()?;
		self.get(id)
	}

	pub fn get(&mut self, id: StreamId) -> Option<&mut PublisherAnnounce> {
		self.lookup.get_mut(&id)
	}
}

pub struct PublisherAnnounce {
	stream: StreamId,
	request: message::AnnouncePlease,
	events: VecDeque<message::Announce>,
	live: bool,
}

impl PublisherAnnounce {
	fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			stream,
			request,
			events: VecDeque::new(),
			live: false,
		}
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		while let Some(event) = self.events.pop_front() {
			event.encode(buf);
		}
	}

	pub fn stream(&self) -> StreamId {
		self.stream
	}

	pub fn info(&mut self) -> &message::AnnouncePlease {
		&self.request
	}

	fn reply(&mut self, msg: message::Announce) {
		self.events.push_back(msg);
	}

	pub fn active(&mut self, path: &str) {
		// TODO: assert! !active
		self.reply(message::Announce::Active(path.to_string()));
	}

	pub fn ended(&mut self, path: &str) {
		// TODO: assert! active
		self.reply(message::Announce::Ended(path.to_string()));
	}

	pub fn live(&mut self) {
		assert!(!self.live);
		self.live = true;
		self.reply(message::Announce::Live);
	}
}
