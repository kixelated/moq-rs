use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self, GroupOrder},
};

use super::{AnnounceId, Error, ErrorCode, GroupId, Increment, StreamDirection, StreamId, SubscribeId};

pub(super) enum SubscriberStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Debug)]
pub enum SubscriberEvent {
	/// An announcement has new data.
	///
	/// Call `announce(id)` with the announcement ID to learn more.
	Announce(AnnounceId),

	/// A subscription has new data.
	///
	/// Call `subscribe(id)` with the subscription ID to learn more.
	Subscribe(SubscribeId),

	/// A group has new data.
	///
	/// Call `group(id)` with the group ID to learn more.
	Group(GroupId),
}

#[derive(Default)]
pub(super) struct SubscriberState {
	announced: HashMap<AnnounceId, SubscriberAnnounceState>,
	announced_next: AnnounceId,
	announced_ready: BTreeSet<AnnounceId>,

	subscribe: HashMap<SubscribeId, SubscriberSubscribeState>,
	subscribe_next: SubscribeId,
	subscribe_ready: BTreeSet<SubscribeId>,

	groups: HashMap<(SubscribeId, GroupId), SubscriberGroupState>,
	groups_ready: BTreeSet<(SubscribeId, GroupId)>,
}

impl SubscriberState {
	pub fn encode<B: BufMut>(&mut self, kind: SubscriberStream, buf: &mut B) {
		match kind {
			SubscriberStream::Announce(id) => self.announced.get_mut(&id).unwrap().encode(buf),
			SubscriberStream::Subscribe(id) => self.subscribe.get_mut(&id).unwrap().encode(buf),
			SubscriberStream::Group(_) => unreachable!("read only"),
		}
	}

	pub fn decode<B: Buf>(&mut self, kind: SubscriberStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			SubscriberStream::Announce(id) => {
				self.announced.get_mut(&id).unwrap().decode(buf)?;
				self.announced_ready.insert(id);
			}
			SubscriberStream::Subscribe(id) => {
				self.subscribe.get_mut(&id).unwrap().decode(buf)?;
				self.subscribe_ready.insert(id);
			}
			SubscriberStream::Group(id) => {
				self.groups.get_mut(&id).unwrap().decode(buf)?;
				self.groups_ready.insert(id);
			}
		}

		Ok(())
	}

	pub fn open(&mut self, stream: StreamId, kind: SubscriberStream) {
		match kind {
			SubscriberStream::Announce(id) => self.announced.get_mut(&id).unwrap().stream = Some(stream),
			SubscriberStream::Subscribe(id) => self.subscribe.get_mut(&id).unwrap().stream = Some(stream),
			SubscriberStream::Group(_) => unreachable!("publisher opens"),
		}
	}

	pub fn accept_group<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<SubscriberStream, Error> {
		let group = message::Group::decode(buf)?;
		let id = (group.subscribe.into(), group.sequence.into());

		let group = SubscriberGroupState::default();
		self.groups.insert(id, group);
		self.groups_ready.insert(id);

		Ok(SubscriberStream::Group(id))
	}
}

struct SubscriberAnnounceState {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
	stream: Option<StreamId>,
}

impl SubscriberAnnounceState {
	fn new(request: message::AnnouncePlease) -> Self {
		Self {
			request: Some(request),
			events: VecDeque::new(),
			stream: None,
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
}

struct SubscriberSubscribeState {
	stream: Option<StreamId>,

	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,

	groups_ready: VecDeque<GroupId>,
}

impl SubscriberSubscribeState {
	pub fn new(request: message::Subscribe) -> Self {
		Self {
			stream: None,
			request: Some(request),
			update: None,

			info: None,
			drops: VecDeque::new(),
			groups_ready: VecDeque::new(),
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
}

#[derive(Default)]
struct SubscriberGroupState {
	frames: VecDeque<usize>,
	data: VecDeque<u8>,

	write_remain: usize,
	read_remain: usize,
}

impl SubscriberGroupState {
	/// Decode the next frame from the stream.
	pub fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		if self.write_remain == 0 {
			let frame = message::Frame::decode(buf)?;
			self.write_remain = frame.size;
		}

		let size = buf.remaining().min(self.write_remain);
		let mut buf = buf.take(size);
		while buf.has_remaining() {
			let chunk = buf.chunk();
			self.data.extend(chunk);
			buf.advance(chunk.len());
		}

		Ok(())
	}

	/// Read the next chunk of the frame if available, returning the remaining size until the next frame.
	pub fn read<B: BufMut>(&mut self, buf: &mut B) -> Option<usize> {
		if self.read_remain == 0 {
			match self.frames.pop_front() {
				Some(size) => self.read_remain = size,
				None => return None,
			}
		}

		let size = buf.remaining_mut().min(self.read_remain);
		buf.limit(size).put(&mut self.data);

		self.read_remain -= size;
		Some(self.read_remain)
	}
}

// message::Subscribe but without the ID.
pub struct SubscribeRequest {
	pub path: String,
	pub priority: i8,
	pub group_order: GroupOrder,
	pub group_min: Option<GroupId>,
	pub group_max: Option<GroupId>,
}

impl SubscribeRequest {
	fn into_message(self, id: u64) -> message::Subscribe {
		message::Subscribe {
			id,
			path: self.path,
			priority: self.priority,
			group_order: self.group_order,
			group_min: self.group_min.map(Into::into),
			group_max: self.group_max.map(Into::into),
		}
	}
}

impl From<message::Subscribe> for SubscribeRequest {
	fn from(msg: message::Subscribe) -> Self {
		Self {
			path: msg.path,
			priority: msg.priority,
			group_order: msg.group_order,
			group_min: msg.group_min.map(Into::into),
			group_max: msg.group_max.map(Into::into),
		}
	}
}
