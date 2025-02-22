use std::collections::HashMap;

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::{Decode, Encode},
	message::{self, GroupOrder, Info, Path},
};

use super::{AnnounceId, Error, FrameId, GroupId, StreamId, SubscribeId};

pub enum AnnounceEvent {
	Active(String),
	Ended(String),
	Live,
}

pub enum GroupEvent {
	Start,
	Frame(FrameId, FrameEvent),
	Final,
	Error(Error),
}

pub enum FrameEvent {
	Size(usize),
	Chunk(Bytes),
	Final,
	Error(Error),
}

#[derive(Default)]
pub struct Subscriber {
	// Waiting for a stream to be created.
	announced: HashMap<AnnounceId, Announce>,

	subscribe: HashMap<SubscribeId, Subscribe>,
	subscribe_next: SubscribeId,
	subscribe_ready: VecDeque<SubscribeId>,

	streams: HashMap<StreamId, Stream>,
}

enum Stream {
	Announce(AnnounceId),
	Subscribe(SubscribeId),
}

impl Subscriber {
	/// Request any tracks matching the specified path.
	pub fn announced(&mut self, path: &str) -> AnnounceId {
		todo!();
	}

	/// Return the next announcement with at least one event ready.
	pub fn announced_ready(&mut self) -> Option<AnnounceId> {
		todo!();
	}

	/// Return the next announced event, if any
	pub fn announced_event(&mut self, id: AnnounceId) -> Option<AnnounceEvent> {
		todo!();
	}

	/// Stop receiving announcements.
	pub fn announced_close(&mut self, id: AnnounceId) -> Result<(), Error> {
		todo!()
	}

	// Start a subscription.
	pub fn subscribe(&mut self, stream_id: StreamId, request: SubscribeRequest) -> &mut Subscribe {
		let subscribe_id = self.subscribe_next;

		let request = message::Subscribe {
			id: subscribe_id.into(),
			path: request.path,
			priority: request.priority,
			group_order: request.group_order,
			group_min: request.group_min,
			group_max: request.group_max,
		};

		self.subscribe_next.incr();
		self.subscribe_ready.push_back(subscribe_id);

		let subscribe = Subscribe::new(stream_id, request);
		self.subscribe.entry(subscribe_id).or_insert(subscribe)
	}

	// Returns a Subscribe
	pub fn subscribe_ready(&mut self) -> Option<&mut Subscribe> {
		todo!();
	}
}

pub struct SubscribeRequest {
	pub path: Path,
	pub priority: i8,
	pub group_order: GroupOrder,
	pub group_min: Option<u64>,
	pub group_max: Option<u64>,
}

struct Subscribe {
	stream: StreamId,
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,
}

pub enum SubscribeEvent {
	Start(Info),
	Group(GroupId, GroupEvent),
	Final,
	Error(Error),
}

impl Subscribe {
	fn new(stream_id: StreamId, request: message::Subscribe) -> Self {
		Self {
			stream: stream_id,
			request: Some(request),
			update: None,
		}
	}

	pub fn event(&mut self) -> Option<SubscribeEvent> {
		todo!();
	}

	pub fn update(&mut self, update: message::SubscribeUpdate) {
		self.update = Some(update);
	}

	pub fn encode<B: BufMut>(&mut self, b: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(b);
		} else if let Some(update) = self.update.take() {
			update.encode(b);
		}
	}
}

#[derive(Default)]
enum Group {
	#[default]
	Init,
	Active(message::Group),
	Closed(Error),
}
