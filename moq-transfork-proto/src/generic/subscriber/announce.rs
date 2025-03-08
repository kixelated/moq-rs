use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	generic::{AnnounceId, Error, Increment, StreamDirection, StreamId, StreamsState},
	message::{self},
};

use super::SubscriberStream;

#[derive(Default)]
pub(super) struct SubscriberAnnouncesState {
	lookup: HashMap<AnnounceId, SubscriberAnnounceState>,
	ready: BTreeSet<AnnounceId>,

	// The announces that are waiting for a stream to be opened.
	blocked: BTreeSet<AnnounceId>,

	next: AnnounceId,
}

impl SubscriberAnnouncesState {
	pub fn decode<B: Buf>(&mut self, id: AnnounceId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)
	}

	pub fn encode<B: BufMut>(&mut self, id: AnnounceId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub fn open(&mut self, stream: StreamId) -> Option<SubscriberStream> {
		if let Some(id) = self.blocked.pop_first() {
			self.lookup.get_mut(&id).unwrap().open(stream);
			Some(SubscriberStream::Announce(id))
		} else {
			None
		}
	}
}

pub struct SubscriberAnnounces<'a> {
	pub(super) state: &'a mut SubscriberAnnouncesState,
	pub(super) streams: &'a mut StreamsState,
}

impl SubscriberAnnounces<'_> {
	pub fn create(&mut self, request: message::AnnouncePlease) -> SubscriberAnnounce {
		let id = self.state.next;
		self.state.next.increment();

		let stream = self.streams.open(SubscriberStream::Announce(id).into());
		if stream.is_none() {
			self.state.blocked.insert(id);
		}

		let announce = SubscriberAnnounceState::new(request, stream);
		let state = self.state.lookup.entry(id).or_insert(announce);
		self.state.ready.insert(id);

		SubscriberAnnounce {
			id,
			state,
			streams: self.streams,
		}
	}

	pub fn get(&mut self, id: AnnounceId) -> Option<SubscriberAnnounce> {
		Some(SubscriberAnnounce {
			id,
			state: self.state.lookup.get_mut(&id)?,
			streams: self.streams,
		})
	}
}

struct SubscriberAnnounceState {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
	stream: Option<StreamId>,
}

impl SubscriberAnnounceState {
	pub fn new(request: message::AnnouncePlease, stream: Option<StreamId>) -> Self {
		Self {
			request: Some(request),
			events: VecDeque::new(),
			stream,
		}
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

	pub fn open(&mut self, stream: StreamId) {
		assert!(self.stream.is_none());
		self.stream = Some(stream);
	}
}

pub struct SubscriberAnnounce<'a> {
	id: AnnounceId,
	state: &'a mut SubscriberAnnounceState,
	streams: &'a mut StreamsState,
}

impl SubscriberAnnounce<'_> {
	pub fn id(&self) -> AnnounceId {
		self.id
	}

	/// Returns the next announcement with pending data.
	pub fn poll(&mut self) -> Option<message::Announce> {
		self.state.events.pop_front()
	}
}
