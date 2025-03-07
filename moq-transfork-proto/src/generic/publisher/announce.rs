use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	generic::{
		AnnounceId, Error, Increment, StreamId,
		StreamsState,
	},
	message::{self},
};

#[derive(Default)]
pub(super) struct PublisherAnnouncesState {
	lookup: HashMap<AnnounceId, PublisherAnnounceState>,
	ready: BTreeSet<AnnounceId>,
	next: AnnounceId,
}

impl PublisherAnnouncesState {
	pub fn accept<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<AnnounceId, Error> {
		let announce = message::AnnouncePlease::decode(buf)?;
		let id = self.next;
		self.next.increment();

		let announce = PublisherAnnounceState::new(stream, announce);
		self.lookup.insert(id, announce);
		self.ready.insert(id);

		Ok(id)
	}

	pub fn decode<B: Buf>(&mut self, id: AnnounceId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)?;
		self.ready.insert(id);

		Ok(())
	}

	pub fn encode<B: BufMut>(&mut self, id: AnnounceId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}
}

pub struct PublisherAnnounces<'a> {
	pub(super) state: &'a mut PublisherAnnouncesState,
	pub(super) streams: &'a mut StreamsState,
}

impl PublisherAnnounces<'_> {
	pub fn accept(&mut self) -> Option<PublisherAnnounce> {
		let id = self.state.ready.pop_first()?;
		Some(PublisherAnnounce {
			id,
			state: self.state.lookup.get_mut(&id).unwrap(),
			streams: self.streams,
		})
	}

	pub fn get(&mut self, id: AnnounceId) -> Option<&mut PublisherAnnounceState> {
		self.state.lookup.get_mut(&id)
	}
}

pub(super) struct PublisherAnnounceState {
	stream: StreamId,
	request: message::AnnouncePlease,
	events: VecDeque<message::Announce>,
	live: bool,
}

impl PublisherAnnounceState {
	pub fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			stream,
			request,
			events: VecDeque::new(),
			live: false,
		}
	}

	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let msg = message::Announce::decode(buf)?;
		self.events.push_back(msg);

		Ok(())
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		while let Some(event) = self.events.pop_front() {
			event.encode(buf);
		}
	}
}

pub struct PublisherAnnounce<'a> {
	pub(super) id: AnnounceId,
	pub(super) state: &'a mut PublisherAnnounceState,
	pub(super) streams: &'a mut StreamsState,
}

impl PublisherAnnounce<'_> {
	pub fn id(&self) -> AnnounceId {
		self.id
	}

	pub fn request(&mut self) -> &message::AnnouncePlease {
		&self.state.request
	}

	fn reply(&mut self, msg: message::Announce) {
		self.state.events.push_back(msg);
		self.streams.encodable(self.state.stream);
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
		assert!(!self.state.live);
		self.state.live = true;
		self.reply(message::Announce::Live);
	}
}
