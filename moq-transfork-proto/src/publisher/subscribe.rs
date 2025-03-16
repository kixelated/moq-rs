use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	message::{self},
	Error, ErrorCode, GroupId, StreamId, SubscribeId, SubscribeRequest,
};

#[derive(Default)]
pub struct PublisherSubscribes {
	lookup: HashMap<SubscribeId, PublisherSubscribe>,
	ready: BTreeSet<SubscribeId>,
	next: SubscribeId,
}

impl PublisherSubscribes {
	pub(crate) fn accept<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<SubscribeId, Error> {
		let subscribe = message::Subscribe::decode(buf)?;
		let id = self.next;
		self.next.increment();

		let subscribe = PublisherSubscribe::new(id, stream, subscribe.into());
		self.lookup.insert(id, subscribe);
		self.ready.insert(id);

		Ok(id)
	}

	pub(crate) fn decode<B: Buf>(&mut self, id: SubscribeId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)?;
		self.ready.insert(id);

		Ok(())
	}

	pub(crate) fn encode<B: BufMut>(&mut self, id: SubscribeId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}

	pub fn accept(&mut self) -> Option<&mut PublisherSubscribe> {
		let id = self.ready.pop_first()?;
		self.lookup.get_mut(&id)
	}

	pub fn get(&mut self, id: SubscribeId) -> Option<&mut PublisherSubscribe> {
		self.lookup.get_mut(&id)
	}
}

struct PublisherSubscribe {
	id: SubscribeId,

	// Outbound
	info: Option<message::Info>,
	info_sent: bool,
	dropped: VecDeque<(GroupId, ErrorCode)>,

	// Inbound
	request: SubscribeRequest,
	update: Option<message::SubscribeUpdate>,

	stream: StreamId,
}

impl PublisherSubscribe {
	pub(crate) fn new(id: SubscribeId, stream: StreamId, request: SubscribeRequest) -> Self {
		Self {
			id,
			info: None,
			info_sent: false,
			dropped: VecDeque::new(),
			stream,
			request,
			update: None,
		}
	}

	pub(crate) fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(info) = self.info.take() {
			info.encode(buf);
		}

		loop {
			let (id, code) = match self.dropped.pop_front() {
				Some(id) => id,
				None => return,
			};

			let mut msg = message::GroupDrop {
				sequence: id.0,
				count: 0,
				code: code.into(),
			};

			while let Some((id, code)) = self.dropped.front().cloned() {
				if msg.sequence + msg.count + 1 == id.0 && msg.code == code.into() {
					msg.count += 1;
					self.dropped.pop_front();
				} else {
					break;
				}
			}

			msg.encode(buf);
		}
	}

	pub(crate) fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let update = message::SubscribeUpdate::decode(buf)?;
		self.update = Some(update);

		Ok(())
	}

	pub fn id(&self) -> SubscribeId {
		self.id
	}

	pub fn info(&mut self) -> &SubscribeRequest {
		&self.request
	}

	pub fn start(&mut self, info: message::Info) {
		assert!(self.info.is_none());
		assert!(!self.info_sent);

		self.info = Some(info);
		self.streams.encodable(self.stream);
	}
}
