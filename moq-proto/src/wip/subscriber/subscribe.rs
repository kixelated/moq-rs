use std::collections::{HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, StreamId, SubscribeId, SubscribeRequest,
};

#[derive(Default)]
pub struct SubscriberSubscribes {
	lookup: HashMap<StreamId, SubscriberSubscribe>,
	next: SubscribeId,
}

impl SubscriberSubscribes {
	pub fn create(&mut self, stream: StreamId, request: SubscribeRequest) -> &mut SubscriberSubscribe {
		let id = self.next;
		self.next.increment();

		let msg = request.into_message(id.0);
		let subscribe = SubscriberSubscribe::new(id, stream, msg);

		self.lookup.entry(stream).or_insert(subscribe)
	}

	pub fn get(&mut self, stream: StreamId) -> Option<&mut SubscriberSubscribe> {
		self.lookup.get_mut(&stream)
	}
}

pub struct SubscriberSubscribe {
	id: SubscribeId,
	stream: StreamId,

	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,
}

impl SubscriberSubscribe {
	pub(crate) fn new(id: SubscribeId, stream: StreamId, request: message::Subscribe) -> Self {
		Self {
			id,
			stream,

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

	/// Try to decode the next message from the stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if self.info.is_none() {
			self.info = Some(message::Info::decode(buf)?);
			return Ok(());
		}

		let dropped = message::GroupDrop::decode(buf)?;
		self.drops.push_back(dropped);

		Ok(())
	}

	pub fn stream(&self) -> StreamId {
		self.stream
	}

	/// Update the subscription with a new priority/ordering.
	///
	/// Must call `encode()` after updating.
	pub fn update(&mut self, update: message::SubscribeUpdate) {
		self.update = Some(update);
	}

	/// Return information about the track if received.
	pub fn info(&mut self) -> Option<&message::Info> {
		self.info.as_ref()
	}

	pub fn dropped(&mut self) -> Option<message::GroupDrop> {
		self.drops.pop_front()
	}
}
