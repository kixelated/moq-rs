use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, Encode},
	generic::{
		Error, ErrorCode, GroupId, Increment, StreamId, StreamsState, SubscribeId, SubscribeRequest,
	},
	message::{self},
};

use super::PublisherGroupState;

#[derive(Default)]
pub(super) struct PublisherSubscribesState {
	lookup: HashMap<SubscribeId, PublisherSubscribeState>,
	ready: BTreeSet<SubscribeId>,
	next: SubscribeId,
}

impl PublisherSubscribesState {
	pub fn accept<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<SubscribeId, Error> {
		let subscribe = message::Subscribe::decode(buf)?;
		let id = self.next;
		self.next.increment();

		let subscribe = PublisherSubscribeState::new(stream, subscribe.into());
		self.lookup.insert(id, subscribe);
		self.ready.insert(id);

		Ok(id)
	}

	pub fn decode<B: Buf>(&mut self, id: SubscribeId, buf: &mut B) -> Result<(), Error> {
		self.lookup.get_mut(&id).unwrap().decode(buf)?;
		self.ready.insert(id);

		Ok(())
	}

	pub fn encode<B: BufMut>(&mut self, id: SubscribeId, buf: &mut B) {
		self.lookup.get_mut(&id).unwrap().encode(buf);
	}
}

pub struct PublisherSubscribes<'a> {
	pub(super) state: &'a mut PublisherSubscribesState,
	pub(super) streams: &'a mut StreamsState,
}

impl PublisherSubscribes<'_> {
	pub fn accept(&mut self) -> Option<PublisherSubscribe> {
		let id = self.state.ready.pop_first()?;
		Some(PublisherSubscribe {
			id,
			state: self.state.lookup.get_mut(&id).unwrap(),
			streams: self.streams,
		})
	}

	pub fn get(&mut self, id: SubscribeId) -> Option<PublisherSubscribe> {
		Some(PublisherSubscribe {
			id,
			state: self.state.lookup.get_mut(&id).unwrap(),
			streams: self.streams,
		})
	}
}

struct PublisherSubscribeState {
	// Outbound
	info: Option<message::Info>,
	info_sent: bool,
	dropped: VecDeque<(GroupId, ErrorCode)>,
	groups: HashMap<GroupId, PublisherGroupState>,

	// Inbound
	request: SubscribeRequest,
	update: Option<message::SubscribeUpdate>,

	stream: StreamId,
}

impl PublisherSubscribeState {
	pub fn new(stream: StreamId, request: SubscribeRequest) -> Self {
		Self {
			info: None,
			info_sent: false,
			dropped: VecDeque::new(),
			groups: HashMap::new(),
			stream,
			request,
			update: None,
		}
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
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

	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let update = message::SubscribeUpdate::decode(buf)?;
		self.update = Some(update);

		Ok(())
	}

	pub fn dropped(&mut self, group: GroupId, error: ErrorCode) {
		self.dropped.push_back((group, error));
	}
}

pub struct PublisherSubscribe<'a> {
	id: SubscribeId,
	state: &'a mut PublisherSubscribeState,
	streams: &'a mut StreamsState,
}

impl PublisherSubscribe<'_> {
	pub fn id(&self) -> SubscribeId {
		self.id
	}

	pub fn requested(&mut self) -> &SubscribeRequest {
		&self.state.request
	}

	pub fn info(&mut self, info: message::Info) {
		assert!(self.state.info.is_none());
		assert!(!self.state.info_sent);

		self.state.info = Some(info);
		self.streams.encodable(self.state.stream);
	}
}
