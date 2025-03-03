use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self, GroupOrder},
};

use super::{
	AnnounceId, Connection, Error, ErrorCode, GroupId, Increment, StreamDirection, StreamId, StreamKind, SubscribeId,
	SubscribeRequest,
};

#[derive(Debug)]
pub enum PublisherEvent {
	/// An announcement is requested.
	///
	/// Call `announce(id)` with the announcement ID and reply with available tracks.
	Announce(AnnounceId),

	/// A subscription is requested.
	///
	/// Call `subscribe(id)` with the subscription ID and reply to the request.
	Subscribe(SubscribeId),
}

pub(super) enum PublisherStream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
	Group((SubscribeId, GroupId)),
}

#[derive(Default)]
pub(super) struct PublisherState {
	announces: HashMap<AnnounceId, PublisherAnnounceState>,
	announced_ready: BTreeSet<AnnounceId>,
	announced_next: AnnounceId,

	subscribes: HashMap<SubscribeId, PublisherSubscribeState>,
	subscribe_ready: BTreeSet<SubscribeId>,
	subscribe_next: SubscribeId,

	groups: HashMap<(SubscribeId, GroupId), PublisherGroupState>,
}

impl PublisherState {
	pub fn encode<B: BufMut>(&mut self, kind: PublisherStream, buf: &mut B) {
		match kind {
			PublisherStream::Announce(id) => self.announces.get_mut(&id).unwrap().encode(buf),
			PublisherStream::Subscribe(id) => self.subscribes.get_mut(&id).unwrap().encode(buf),
			PublisherStream::Group(id) => self.groups.get_mut(&id).unwrap().encode(buf),
		}
	}

	pub fn decode<B: Buf>(&mut self, kind: PublisherStream, buf: &mut B) -> Result<(), Error> {
		match kind {
			PublisherStream::Announce(id) => {
				self.announces.get_mut(&id).unwrap().decode(buf)?;
				self.announced_ready.insert(id);
			}
			PublisherStream::Subscribe(id) => {
				self.subscribes.get_mut(&id).unwrap().decode(buf)?;
				self.subscribe_ready.insert(id);
			}
			PublisherStream::Group(_) => unreachable!("write only"),
		}

		Ok(())
	}

	pub fn open(&mut self, stream: StreamId, kind: PublisherStream) {
		match kind {
			PublisherStream::Group(id) => self.groups.get_mut(&id).unwrap().stream = Some(stream),
			_ => unreachable!("subscriber opens"),
		}
	}

	pub fn accept_announce<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let announce = message::AnnouncePlease::decode(buf)?;
		let id = self.announced_next;
		self.announced_next.increment();

		let announce = PublisherAnnounceState::new(stream, announce);
		self.announces.insert(id, announce);
		self.announced_ready.insert(id);

		Ok(PublisherStream::Announce(id))
	}

	pub fn accept_subscribe<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<PublisherStream, Error> {
		let subscribe = message::Subscribe::decode(buf)?;
		let id = self.subscribe_next;
		self.subscribe_next.increment();

		let subscribe = PublisherSubscribeState::new(stream, subscribe.into());
		self.subscribes.insert(id, subscribe);
		self.subscribe_ready.insert(id);

		Ok(PublisherStream::Subscribe(id))
	}
}

struct PublisherAnnounceState {
	stream: StreamId,
	request: message::AnnouncePlease,
	events: VecDeque<message::Announce>,
}

impl PublisherAnnounceState {
	pub fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			stream,
			request,
			events: VecDeque::new(),
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

struct PublisherSubscribeState {
	// Outbound
	info: Option<message::Info>,
	info_sent: bool,
	dropped: VecDeque<(GroupId, ErrorCode)>,

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
}

#[derive(Default)]
struct PublisherGroupState {
	stream: Option<StreamId>,
	frames: VecDeque<usize>,
	chunks: VecDeque<Bytes>,

	write_remain: usize,
	read_remain: usize,
}

impl PublisherGroupState {
	pub fn frame(&mut self, size: usize) {
		assert_eq!(self.write_remain, 0);
		self.write_remain = size;
		self.frames.push_back(size);
	}

	pub fn write<B: Buf>(&mut self, buf: &mut B) {
		assert!(self.write_remain > buf.remaining());

		// TODO enforce a maximum buffer size
		let chunk = buf.copy_to_bytes(buf.remaining());
		self.write_remain -= chunk.len();
		self.chunks.push_back(chunk);
	}

	pub fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if self.read_remain == 0 {
			let size = match self.frames.pop_front() {
				Some(size) => size,
				None => return,
			};

			self.read_remain = size;

			message::Frame { size }.encode(buf);
		}

		if let Some(mut chunk) = self.chunks.pop_front() {
			let size = buf.remaining_mut().min(self.read_remain);
			buf.limit(size).put(&mut chunk);

			self.read_remain -= size;

			if chunk.len() > size {
				self.chunks.push_front(chunk);
			}
		}
	}
}
