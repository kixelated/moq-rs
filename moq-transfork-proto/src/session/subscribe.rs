use std::collections::{hash_map, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::{Decode, Encode},
	message::{self, GroupOrder, Info, Path},
};

use super::{Error, FrameId, GroupId, Stream, StreamCode, StreamId};
pub struct Subscribe {
	lookup: HashMap<StreamId, SubscribeState>,
	next: u64,
}

impl Subscribe {
	// Start a subscription.
	pub fn start(&mut self, stream_id: StreamId, request: SubscribeRequest) -> Result<(), Error> {
		let subscribe_id = self.next;

		let request = message::Subscribe {
			id: subscribe_id,
			path: request.path,
			priority: request.priority,
			group_order: request.group_order,
			group_min: request.group_min,
			group_max: request.group_max,
		};

		self.next += 1;

		let subscribe = SubscribeState::new(request);
		match self.lookup.entry(stream_id) {
			hash_map::Entry::Vacant(entry) => entry.insert(subscribe),
			hash_map::Entry::Occupied(_) => return Err(Error::DuplicateStream),
		};

		Ok(())
	}

	pub fn update(&mut self, stream_id: StreamId, update: SubscribeUpdate) -> Result<(), Error> {
		self.lookup.get_mut(&stream_id).ok_or(Error::UnknownStream)?.update = Some(update);
		Ok(())
	}

	pub fn event(&mut self) -> Option<(StreamId, SubscribeEvent)> {
		todo!();
	}

	pub fn stop(&mut self, stream_id: StreamId) -> Result<(), Error> {
		//self.lookup.remove(&stream_id);
		todo!()
	}
}

impl Stream for Subscribe {
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {}

	fn encode<B: BufMut>(&mut self, b: &mut B) {}

	fn close(&mut self, code: StreamCode) {}

	fn closed(&mut self) -> Option<StreamCode> {}
}

// message::Subscribe but without an ID.
pub struct SubscribeRequest {
	pub path: Path,
	pub priority: i8,
	pub group_order: GroupOrder,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

pub type SubscribeUpdate = message::SubscribeUpdate;
pub type SubscribeInfo = message::Info;

struct SubscribeState {
	// To encode.
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,
	closed: Option<StreamCode>,

	// To return.
	info: Option<message::Info>,
	dropped: VecDeque<message::GroupDrop>,
}

impl SubscribeState {
	fn new(request: message::Subscribe) -> Self {
		Self {
			request: Some(request),
			update: None,
			closed: None,
			info: None,
			dropped: VecDeque::new(),
		}
	}

	pub fn info(&self) -> Option<SubscribeInfo> {
		self.info.clone()
	}

	pub fn dropped(&mut self) -> Option<(GroupId, StreamCode)> {
		let mut drop = self.dropped.pop_front()?;
		let code = drop.code.into();
		let group = drop.sequence.into();

		if drop.count > 0 {
			drop.count -= 1;
			drop.sequence += 1;
			self.dropped.push_front(drop);
		}

		Some((group, code))
	}
}

impl Stream for SubscribeState {
	fn encode<B: BufMut>(&mut self, b: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(b);
		} else if let Some(update) = self.update.take() {
			update.encode(b);
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let _drop = message::GroupDrop::decode(buf)?;
		// TODO use
		Ok(())
	}

	fn closed(&mut self) -> Option<u8> {
		self.closed.clone()
	}

	fn close(&mut self, code: u8) {
		self.closed = Some(code);
	}
}
