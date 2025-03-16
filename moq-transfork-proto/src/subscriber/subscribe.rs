use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, StreamId, StreamsState, SubscribeId, SubscribeRequest,
};

use super::SubscriberStream;

#[derive(Default)]
pub(super) struct SubscriberSubscribesState {
	lookup: HashMap<SubscribeId, SubscriberSubscribeState>,
	ready: BTreeSet<SubscribeId>,
	next: SubscribeId,

	// The subscribes that are waiting for a stream to be opened.
	open: BTreeSet<SubscribeId>, // TODO sort by priority
}

impl SubscriberSubscribesState {
	pub fn decode<B: Buf>(&mut self, id: SubscribeId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)
	}

	pub fn encode<B: BufMut>(&mut self, id: SubscribeId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub fn open(&mut self, stream: StreamId) -> Option<SubscriberStream> {
		if let Some(id) = self.open.pop_first() {
			self.lookup.get_mut(&id).unwrap().open(stream);
			Some(SubscriberStream::Subscribe(id))
		} else {
			None
		}
	}
}

pub struct SubscriberSubscribes<'a> {
	pub(super) state: &'a mut SubscriberSubscribesState,
	pub(super) streams: &'a mut StreamsState,
}

impl SubscriberSubscribes<'_> {
	pub fn create(&mut self, request: SubscribeRequest) -> SubscriberSubscribe {
		let id = self.state.next;
		self.state.next.increment();

		self.streams.open(SubscriberStream::Subscribe(id).into());
		self.state.open.insert(id);

		let msg = request.into_message(id.0);
		let subscribe = SubscriberSubscribeState::new(msg);

		let state = self.state.lookup.entry(id).or_insert(subscribe);
		self.state.ready.insert(id);

		SubscriberSubscribe {
			id,
			state,
			streams: self.streams,
		}
	}

	pub fn get(&mut self, id: SubscribeId) -> Option<SubscriberSubscribe> {
		Some(SubscriberSubscribe {
			id,
			state: self.state.lookup.get_mut(&id)?,
			streams: self.streams,
		})
	}
	/// Returns the next subscription with pending data.
	pub fn ready(&mut self) -> Option<SubscriberSubscribe> {
		let id = self.state.ready.pop_first()?;
		self.get(id)
	}
}

pub(super) struct SubscriberSubscribeState {
	stream: Option<StreamId>,

	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,
}

impl SubscriberSubscribeState {
	pub fn new(request: message::Subscribe) -> Self {
		Self {
			stream: None,
			request: Some(request),
			update: None,

			info: None,
			drops: VecDeque::new(),
		}
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(buf);
		}

		if let Some(update) = self.update.take() {
			update.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if self.info.is_none() {
			self.info = Some(message::Info::decode(buf)?);
			return Ok(());
		}

		let dropped = message::GroupDrop::decode(buf)?;
		self.drops.push_back(dropped);

		Ok(())
	}

	pub fn open(&mut self, stream: StreamId) {
		assert!(self.stream.is_none());
		self.stream = Some(stream);
	}
}

pub struct SubscriberSubscribe<'a> {
	id: SubscribeId,
	state: &'a mut SubscriberSubscribeState,
	streams: &'a mut StreamsState,
}

impl SubscriberSubscribe<'_> {
	pub fn id(&self) -> SubscribeId {
		self.id
	}

	/// Update the subscription with a new priority/ordering.
	pub fn update(&mut self, update: message::SubscribeUpdate) {
		self.state.update = Some(update);

		if let Some(stream) = self.state.stream {
			self.streams.encodable(stream);
		}
	}

	/// Return information about the track if received.
	pub fn info(&mut self) -> Option<&message::Info> {
		self.state.info.as_ref()
	}

	pub fn dropped(&mut self) -> Option<message::GroupDrop> {
		self.state.drops.pop_front()
	}
}
