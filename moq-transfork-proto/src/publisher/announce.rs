use std::{
	collections::{BTreeSet, HashMap, VecDeque},
	fmt,
};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message, AnnounceId, Error, StreamId,
};

#[derive(Default)]
pub struct PublisherAnnounces {
	lookup: HashMap<AnnounceId, PublisherAnnounce>,
	ready: BTreeSet<AnnounceId>,
	next: AnnounceId,
}

impl PublisherAnnounces {
	pub(crate) fn create<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<AnnounceId, Error> {
		let announce = message::AnnouncePlease::decode(buf)?;
		let id = self.next;
		self.next.increment();

		let announce = PublisherAnnounce::new(id, stream, announce);
		self.lookup.insert(id, announce);
		self.ready.insert(id);

		Ok(id)
	}

	pub(crate) fn decode<B: Buf>(&mut self, id: AnnounceId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)?;
		self.ready.insert(id);

		Ok(())
	}

	pub(crate) fn encode<B: BufMut>(&mut self, id: AnnounceId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub fn accept(&mut self) -> Option<&mut PublisherAnnounce> {
		let id = self.ready.pop_first()?;
		self.get(id)
	}

	pub fn get(&mut self, id: AnnounceId) -> Option<&mut PublisherAnnounce> {
		self.lookup.get_mut(&id)
	}
}

pub struct PublisherAnnounce {
	id: AnnounceId,
	stream: StreamId,
	request: message::AnnouncePlease,
	events: VecDeque<message::Announce>,
	live: bool,
}

impl PublisherAnnounce {
	pub(crate) fn new(id: AnnounceId, stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			id,
			stream,
			request,
			events: VecDeque::new(),
			live: false,
		}
	}

	pub(crate) fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let msg = message::Announce::decode(buf)?;
		self.events.push_back(msg);

		Ok(())
	}

	pub(crate) fn encode<B: BufMut>(&mut self, buf: &mut B) {
		while let Some(event) = self.events.pop_front() {
			event.encode(buf);
		}
	}

	pub fn id(&self) -> AnnounceId {
		self.id
	}

	pub fn info(&mut self) -> &message::AnnouncePlease {
		&self.request
	}

	fn reply(&mut self, msg: message::Announce) {
		self.events.push_back(msg);
		self.streams.encodable(self.stream);
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
