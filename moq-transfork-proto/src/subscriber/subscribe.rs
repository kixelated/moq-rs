use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, StreamId, SubscribeId, SubscribeRequest,
};

#[derive(Default)]
pub struct SubscriberSubscribes {
	lookup: HashMap<SubscribeId, SubscriberSubscribe>,
	ready: BTreeSet<SubscribeId>,
	next: SubscribeId,

	// The subscribes that are waiting for a stream to be opened.
	blocked: BTreeSet<SubscribeId>, // TODO sort by priority
}

impl SubscriberSubscribes {
	pub(crate) fn decode<B: Buf>(&mut self, id: SubscribeId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)
	}

	pub(crate) fn encode<B: BufMut>(&mut self, id: SubscribeId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub fn open(&mut self, stream: StreamId) -> &mut SubscriberSubscribe {
		let id = self.blocked.pop_first().expect("no blocked subscriptions");
		let entry = self.lookup.get_mut(&id).unwrap();
		entry.open(stream);
		entry
	}

	pub fn create(&mut self, request: SubscribeRequest) -> &mut SubscriberSubscribe {
		let id = self.next;
		self.next.increment();

		self.blocked.insert(id);

		let msg = request.into_message(id.0);
		let subscribe = SubscriberSubscribe::new(id, msg);

		self.lookup.entry(id).or_insert(subscribe)
	}

	pub fn get(&mut self, id: SubscribeId) -> Option<&mut SubscriberSubscribe> {
		self.lookup.get_mut(&id)
	}
	/// Returns the next subscription with pending data.
	pub fn ready(&mut self) -> Option<&mut SubscriberSubscribe> {
		let id = self.ready.pop_first()?;
		self.get(id)
	}
}

pub struct SubscriberSubscribe {
	id: SubscribeId,
	stream: Option<StreamId>,

	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,
}

impl SubscriberSubscribe {
	pub(crate) fn new(id: SubscribeId, request: message::Subscribe) -> Self {
		Self {
			id,
			stream: None,

			request: Some(request),
			update: None,

			info: None,
			drops: VecDeque::new(),
		}
	}

	pub(crate) fn encode<B: BufMut>(&mut self, buf: &mut B) {
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

	fn open(&mut self, stream: StreamId) {
		assert!(self.stream.is_none());
		self.stream = Some(stream);
	}

	pub fn id(&self) -> SubscribeId {
		self.id
	}

	/// Update the subscription with a new priority/ordering.
	pub fn update(&mut self, update: message::SubscribeUpdate) {
		self.update = Some(update);
		todo!("mark as encodable");
	}

	/// Return information about the track if received.
	pub fn info(&mut self) -> Option<&message::Info> {
		self.info.as_ref()
	}

	pub fn dropped(&mut self) -> Option<message::GroupDrop> {
		self.drops.pop_front()
	}
}
