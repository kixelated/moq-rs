use std::collections::{hash_map, BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, DecodeError, Encode},
	message,
};

use super::{AnnounceId, Error, GroupId, Increment, StreamCode, StreamId, SubscribeId};

#[derive(Default)]
pub struct Session {
	publisher: PublisherState,
	subscriber: SubscriberState,
	streams: StreamsState,
}

impl Session {
	pub fn subscriber(&mut self) -> Subscriber {
		Subscriber { session: self }
	}

	pub fn publisher(&mut self) -> Publisher {
		Publisher { session: self }
	}

	pub fn streams(&mut self) -> Streams {
		Streams { session: self }
	}
}

#[derive(Default)]
pub struct StreamsState {
	active: HashMap<StreamId, StreamState>,

	create_bi: VecDeque<StreamKind>,
	create_uni: VecDeque<(SubscribeId, GroupId)>,

	encodable: BTreeSet<StreamId>,
}

pub struct Streams<'a> {
	session: &'a mut Session,
}

impl<'a> Streams<'a> {
	pub fn encode<B: BufMut>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		let stream = self.session.streams.active.get_mut(&id).ok_or(Error::UnknownStream)?;

		if !stream.send_buffer.is_empty() {
			let size = buf.remaining_mut().min(stream.send_buffer.len());
			buf.put_slice(&stream.send_buffer[..size]);

			// TODO This is not efficient.
			// We should use a ring buffer (VecDeque) instead, but it's not BufMut.
			stream.send_buffer.drain(..size);
		}

		let chain = &mut buf.chain_mut(&mut stream.send_buffer);
		match stream.kind {
			StreamKind::PublisherAnnounce(id) => PublisherAnnounce {
				session: self.session,
				id,
			}
			.encode(chain),
			StreamKind::PublisherSubscribe(id) => PublisherSubscribe {
				session: self.session,
				id,
			}
			.encode(chain),
			StreamKind::PublisherGroup(id) => PublisherGroup {
				session: self.session,
				id,
			}
			.encode(chain),
			StreamKind::SubscriberAnnounce(id) => SubscriberAnnounce {
				session: self.session,
				id,
			}
			.encode(chain),
			StreamKind::SubscriberSubscribe(id) => SubscriberSubscribe {
				session: self.session,
				id,
			}
			.encode(chain),
			StreamKind::SubscriberGroup(_) => unreachable!("read only"),
			StreamKind::PublisherRecv | StreamKind::SubscriberRecv => unreachable!(),
		};

		Ok(())
	}

	/// Returns the next stream ID that can be newly encoded.
	pub fn encodable(&mut self) -> Option<StreamId> {
		self.session.streams.encodable.pop_first()
	}

	pub fn decode<B: Buf>(&mut self, stream: StreamId, buf: &mut B) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		let stream = match self.session.streams.active.entry(stream) {
			hash_map::Entry::Occupied(entry) => entry.into_mut(),
			hash_map::Entry::Vacant(entry) => {
				let kind = if stream.is_bi() {
					match message::ControlType::decode(buf)? {
						message::ControlType::Session => todo!(),
						message::ControlType::Announce => StreamKind::PublisherAnnounce,
						message::ControlType::Subscribe => StreamKind::PublisherSubscribe,
						message::ControlType::Info => todo!(),
					}
				} else {
					match message::DataType::decode(buf)? {
						message::DataType::Group => StreamKind::SubscriberGroup,
					}
				};

				entry.insert(StreamState::new(kind))
			}
		};

		let chain = &mut stream.recv_buffer.chain(buf);

		let res = match stream.kind {
			StreamKind::SubscriberAnnounce => SubscriberAnnounce {
				session: self.session,
				id,
			}
			.decode(chain),
			StreamKind::SubscriberSubscribe => SubscriberSubscribe {
				session: self.session,
				id,
			}
			.decode(chain),
			StreamKind::SubscriberGroup => SubscriberGroup {
				session: self.session,
				id,
			}
			.decode(buf),
			StreamKind::PublisherAnnounce => PublisherAnnounce {
				session: self.session,
				id,
			}
			.decode(chain),
			StreamKind::PublisherSubscribe => PublisherSubscribe {
				session: self.session,
				id,
			}
			.decode(chain),
			StreamKind::PublisherGroup => unreachable!("write only"),
		};

		if let Err(Error::Coding(DecodeError::Short)) = res {
			stream.recv_buffer.put(buf);
			Ok(())
		} else {
			res
		}
	}

	pub fn open_bi(&mut self, id: &mut Option<StreamId>) {
		if id.is_none() {
			return;
		}

		let streams = &mut self.session.streams;
		let kind = match streams.create_bi.pop_front() {
			None => return,
			Some(kind) => kind,
		};

		let id = id.take().unwrap();
		let subscriber = &mut self.session.subscriber;

		match kind {
			StreamKind::AnnounceRecv => {
				subscriber.announced.get_mut(&id).unwrap().stream = Some(id);
			}
			StreamKind::SubscribeRecv => {
				subscriber.subscribe.get_mut(&id).unwrap().stream = Some(id);
			}
			_ => unreachable!(),
		};

		streams.encodable.insert(id);
	}

	pub fn open_uni(&mut self, stream: &mut Option<StreamId>) {
		if stream.is_none() {
			return;
		}

		let streams = &mut self.session.streams;
		let id = match streams.create_uni.pop_front() {
			None => return,
			Some(kind) => kind,
		};

		let stream = stream.take().unwrap();
		let publisher = &mut self.session.publisher;

		publisher.groups.get_mut(&id).unwrap().stream = stream;
		streams.encodable.insert(stream);
	}
}

struct StreamState {
	kind: StreamKind,
	send_buffer: Vec<u8>,
	recv_buffer: Vec<u8>,
}

impl StreamState {
	pub fn new(kind: StreamKind) -> Self {
		Self {
			kind,
			send_buffer: Vec::new(),
			recv_buffer: Vec::new(),
		}
	}
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, PartialOrd, Ord)]
enum StreamKind {
	PublisherRecv,
	PublisherAnnounce(AnnounceId),
	PublisherSubscribe(SubscribeId),
	PublisherGroup((SubscribeId, GroupId)),

	SubscriberRecv,
	SubscriberAnnounce(AnnounceId),
	SubscriberSubscribe(SubscribeId),
	SubscriberGroup((GroupId, SubscribeId)),
}

pub struct Subscriber<'a> {
	session: &'a mut Session,
}

#[derive(Default)]
struct SubscriberState {
	announced: HashMap<AnnounceId, SubscriberAnnounceState>,
	announced_create: VecDeque<message::AnnouncePlease>,
	announced_next: AnnounceId,
	announced_ready: BTreeSet<AnnounceId>,

	subscribe: HashMap<SubscribeId, SubscriberSubscribeState>,
	subscribe_next: SubscribeId,
	subscribe_ready: BTreeSet<SubscribeId>,

	groups: HashMap<(SubscribeId, GroupId), GroupRecvState>,
	groups_ready: BTreeSet<(SubscribeId, GroupId)>,
}

impl<'a> Subscriber<'a> {
	// Returns the active announcement with the given ID.
	pub fn announced(&mut self, id: AnnounceId) -> SubscriberAnnounce {
		SubscriberAnnounce {
			session: self.session,
			id,
		}
	}

	// Returns the active subscription with the given ID.
	pub fn subscribe(&mut self, id: SubscribeId) -> SubscriberSubscribe {
		SubscriberSubscribe {
			session: self.session,
			id,
		}
	}

	pub fn create_announced(&mut self, request: message::AnnouncePlease) -> AnnounceId {
		let state = &mut self.session.subscriber;
		let id = state.announced_next;
		state.announced_next.increment();
		state.announced.insert(id, SubscriberAnnounceState::new(request));

		self.session
			.streams
			.create_bi
			.push_back(StreamKind::SubscriberAnnounce(id));

		id
	}

	pub fn create_subscribe(&mut self, request: message::Subscribe) -> SubscribeId {
		let state = &mut self.session.subscriber;
		let id = state.subscribe_next;
		state.subscribe_next.increment();
		state.subscribe.insert(id, SubscriberSubscribeState::new(request));

		self.session
			.streams
			.create_bi
			.push_back(StreamKind::SubscriberSubscribe(id));

		id
	}

	/// Returns the next announcement with pending data.
	pub fn next_announced(&mut self) -> Option<SubscriberAnnounce> {
		let id = self.session.subscriber.announced_ready.pop_first()?;
		Some(SubscriberAnnounce {
			session: self.session,
			id,
		})
	}

	/// Returns the next subscription with pending data.
	pub fn next_subscribe(&mut self) -> Option<SubscriberSubscribe> {
		let id = self.session.subscriber.subscribe_ready.pop_first()?;
		Some(SubscriberSubscribe {
			session: self.session,
			id,
		})
	}
}

struct SubscriberAnnounceState {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
	stream: Option<StreamId>,
}

impl SubscriberAnnounceState {
	pub fn new(request: message::AnnouncePlease) -> Self {
		Self {
			request: Some(request),
			events: VecDeque::new(),
			stream: None,
		}
	}
}

pub struct SubscriberAnnounce<'a> {
	session: &'a mut Session,
	id: AnnounceId,
}

impl<'a> SubscriberAnnounce<'a> {
	fn state(&self) -> &SubscriberAnnounceState {
		self.session.subscriber.announced.get(&self.id).unwrap()
	}

	fn state_mut(&mut self) -> &mut SubscriberAnnounceState {
		self.session.subscriber.announced.get_mut(&self.id).unwrap()
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.state_mut();

		if let Some(request) = state.request.take() {
			request.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		// Can't use state_mut because of the borrow checker
		let state = self.session.subscriber.announced.get_mut(&self.id).unwrap();

		while buf.remaining() > 0 {
			let dropped = message::Announce::decode(buf)?;
			state.events.push_back(dropped);

			self.session.subscriber.announced_ready.insert(self.id);
		}

		Ok(())
	}

	pub fn event(&mut self) -> Option<message::Announce> {
		self.state_mut().events.pop_front()
	}
}

struct SubscriberSubscribeState {
	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,
	groups: VecDeque<GroupId>,

	stream: Option<StreamId>,
}

impl SubscriberSubscribeState {
	pub fn new(request: message::Subscribe) -> Self {
		Self {
			stream: None,
			request: Some(request),
			update: None,

			info: None,
			drops: VecDeque::new(),
			groups: VecDeque::new(),
		}
	}
}

pub struct SubscriberSubscribe<'a> {
	session: &'a mut Session,
	id: SubscribeId,
}

impl<'a> SubscriberSubscribe<'a> {
	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.session.subscriber.subscribe.get_mut(&self.id).unwrap();

		if let Some(request) = state.request.take() {
			request.encode(buf);
		}

		if let Some(update) = state.update.take() {
			update.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let state = self.session.subscriber.subscribe.get_mut(&self.id).unwrap();

		if state.info.is_none() {
			state.info = Some(message::Info::decode(buf)?);
			self.session.subscriber.subscribe_ready.insert(self.id);
		}

		while buf.remaining() > 0 {
			let dropped = message::GroupDrop::decode(buf)?;
			state.drops.push_back(dropped);

			self.session.subscriber.subscribe_ready.insert(self.id);
		}

		Ok(())
	}

	/// Update the subscription with a new priority/ordering.
	pub fn update(&mut self, update: message::SubscribeUpdate) {
		let state = self.session.subscriber.subscribe.get_mut(&self.id).unwrap();
		state.update = Some(update);

		if let Some(stream) = state.stream {
			self.session.streams.encodable.insert(stream);
		}
	}

	/// Return information about the track if received.
	pub fn info(&mut self) -> Option<message::Info> {
		let state = self.session.subscriber.subscribe.get(&self.id).unwrap();
		state.info.clone()
	}

	pub fn dropped(&mut self) -> Option<message::GroupDrop> {
		let state = self.session.subscriber.subscribe.get_mut(&self.id).unwrap();
		state.drops.pop_front()
	}

	/// Return the group with the given ID.
	// TODO return Option?
	pub fn group(&mut self, id: GroupId) -> SubscriberGroup {
		SubscriberGroup {
			id: (self.id, id),
			session: self.session,
		}
	}

	/// Returns the a group stream for the given subscription.
	pub fn group_next(&mut self) -> Option<SubscriberGroup> {
		let state = self.session.subscriber.subscribe.get_mut(&self.id).unwrap();
		let group = state.groups.pop_front()?;
		Some(SubscriberGroup {
			id: (self.id, group),
			session: self.session,
		})
	}
}

pub struct GroupRecvState {
	stream: StreamId,

	frames: VecDeque<usize>,
	data: VecDeque<u8>,

	write_remain: usize,
	read_remain: usize,
}

pub struct SubscriberGroup<'a> {
	session: &'a mut Session,
	id: (SubscribeId, GroupId),
}

impl<'a> SubscriberGroup<'a> {
	fn state(&self) -> &GroupRecvState {
		self.session.subscriber.groups.get(&self.id).unwrap()
	}

	fn state_mut(&mut self) -> &mut GroupRecvState {
		self.session.subscriber.groups.get_mut(&self.id).unwrap()
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let state = self.session.subscriber.groups.get_mut(&self.id).unwrap();

		while buf.has_remaining() {
			if state.write_remain == 0 {
				let frame = message::Frame::decode(buf)?;
				state.write_remain = frame.size;
			}

			let size = buf.remaining().min(state.write_remain);
			state.data.limit(size).put(buf);
		}

		Ok(())
	}

	/// Return the remaining size of the next frame.
	pub fn frame(&mut self) -> Option<usize> {
		let state = self.state_mut();
		if state.read_remain == 0 {
			state.read_remain = state.frames.pop_front()?;
		}

		Some(state.read_remain)
	}

	/// Read the next chunk of the frame if available.
	pub fn read<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.state_mut();

		let size = buf.remaining_mut().min(state.read_remain);
		buf.limit(size).put(&mut state.data);
	}
}

#[derive(Default)]
pub struct PublisherState {
	announces: HashMap<AnnounceId, PublisherAnnounceState>,
	announced_ready: BTreeSet<AnnounceId>,

	subscribes: HashMap<SubscribeId, PublisherSubscribeState>,
	subscribe_ready: BTreeSet<SubscribeId>,

	groups: HashMap<(SubscribeId, GroupId), PublisherGroupState>,
}

pub struct Publisher<'a> {
	session: &'a mut Session,
}

struct PublisherAnnounceState {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
	stream: StreamId,
}

impl PublisherAnnounceState {
	pub fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			stream,
			request: Some(request),
			events: VecDeque::new(),
		}
	}
}

pub struct PublisherAnnounce<'a> {
	session: &'a mut Session,
	id: AnnounceId,
}

impl<'a> PublisherAnnounce<'a> {
	pub fn request(&self) -> &message::AnnouncePlease {
		self.session
			.publisher
			.announces
			.get(&self.id)
			.unwrap()
			.request
			.as_ref()
			.unwrap()
	}

	pub fn reply(&mut self, msg: message::Announce) {
		let state = self.session.publisher.announces.get_mut(&self.id).unwrap();
		state.events.push_back(msg);

		self.session.streams.encodable.insert(state.stream);
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let state = self.session.publisher.announces.get_mut(&self.id).unwrap();
		state.events.push_back(message::Announce::decode(buf)?);

		self.session.streams.encodable.insert(state.stream);

		Ok(())
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.session.publisher.announces.get_mut(&self.id).unwrap();

		while let Some(event) = state.events.pop_front() {
			event.encode(buf);
		}
	}
}

struct PublisherSubscribeState {
	// Outbound
	info: Option<message::Info>,
	info_sent: bool,
	dropped: VecDeque<message::GroupDrop>,

	// Inbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	stream: StreamId,
}

impl PublisherSubscribeState {
	pub fn new(stream: StreamId) -> Self {
		Self {
			info: None,
			info_sent: false,
			dropped: VecDeque::new(),
			stream,
			request: None,
			update: None,
		}
	}
}

pub struct PublisherSubscribe<'a> {
	session: &'a mut Session,
	id: SubscribeId,
}

impl<'a> PublisherSubscribe<'a> {
	pub fn request(&self) -> &message::Subscribe {
		self.session
			.publisher
			.subscribes
			.get(&self.id)
			.unwrap()
			.request
			.as_ref()
			.unwrap()
	}

	pub fn info(&mut self, info: message::Info) {
		let state = self.session.publisher.subscribes.get_mut(&self.id).unwrap();
		assert!(!state.info_sent); // TODO return an error instead
		state.info = Some(info);
		state.info_sent = true;

		self.session.streams.encodable.insert(state.stream);
	}

	pub fn dropped(&mut self, dropped: message::GroupDrop) {
		let state = self.session.publisher.subscribes.get_mut(&self.id).unwrap();
		state.dropped.push_back(dropped);

		self.session.streams.encodable.insert(state.stream);
	}

	pub fn create_group(&mut self, group: GroupId) -> PublisherGroup {
		let id = (self.id, group);

		self.session.publisher.groups.insert(id, PublisherGroupState::default());
		self.session.streams.create_bi.push_back(StreamKind::PublisherGroup(id));

		PublisherGroup {
			session: self.session,
			id,
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.session.publisher.subscribes.get_mut(&self.id).unwrap();

		if let Some(info) = state.info.take() {
			info.encode(buf);
		}

		while let Some(dropped) = state.dropped.pop_front() {
			dropped.encode(buf);
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let state = self.session.publisher.subscribes.get_mut(&self.id).unwrap();

		if state.request.is_none() {
			state.request = Some(message::Subscribe::decode(buf)?);
			self.session.publisher.subscribe_ready.insert(self.id);
		}

		while buf.has_remaining() {
			let update = message::SubscribeUpdate::decode(buf)?;
			state.update = Some(update);
		}

		Ok(())
	}
}

#[derive(Default)]
struct PublisherGroupState {
	frames: VecDeque<usize>,
	remain: usize, // remaining in the current frame.
	buffer: VecDeque<u8>,
}

pub struct PublisherGroup<'a> {
	session: &'a mut Session,
	id: (SubscribeId, GroupId),
}

impl<'a> PublisherGroup<'a> {
	/// Start a new frame with the indicated size.
	pub fn frame(&mut self, size: usize) {
		let state = self.session.publisher.groups.get_mut(&self.id).unwrap();
		state.remain = Some(size);
	}

	pub fn write<B: Buf>(&mut self, buf: &mut B) {
		let state = self.session.publisher.groups.get_mut(&self.id).unwrap();
		state.buffer.put(buf);
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let state = self.session.publisher.groups.get_mut(&self.id).unwrap();

		if let Some(size) = state.remain {
			let size = size.min(buf.remaining_mut());
			buf.put_slice(&state.buffer.drain(..size).collect::<Vec<_>>());
		}
	}
}

/*
pub struct PublisherStatic {
	downstream: Publisher,
	tracks: HashMap<String, PublisherTrack>,

	announces: HashMap<StreamId, message::AnnouncePlease>,
	subscribes: HashMap<String, HashMap<StreamId, message::Subscribe>>,

	cache: Option<Cache>,
}

struct Cache {
	group: usize,
	frames: Vec<CacheFrame>,
}

struct CacheFrame {
	size: usize,
	chunks: Vec<Vec<u8>>,
}

impl PublisherStatic {
	pub fn update(&mut self) {
		while let Some((stream, announce)) = self.downstream.announce_request() {
			for path in self.tracks.keys() {
				if let Some(m) = announce.filter.matches(path) {
					self.downstream
						.announce_reply(stream, message::Announce::Active(m.capture().to_string()));
				}
			}

			self.downstream.announce_reply(stream, message::Announce::Live);

			self.announces.insert(stream, announce);
		}

		while let Some(stream) = self.downstream.announce_closed() {
			self.announces.remove(&stream);
		}

		while let Some((stream, subscribe)) = self.downstream.subscribe_request() {
			if let Some(track) = self.tracks.get(&subscribe.path) {
				self.downstream.subscribe_info(stream, track.info.clone());

				if let Some(cache) = &self.cache {
					self.downstream.subscribe_group(stream, group);

					for frame in &cache.frames {
						self.downstream.subscribe_frame(stream, frame.size);

						for chunk in &frame.chunks {
							self.downstream.subscribe_chunk(stream, chunk);
						}
					}
				}

				self.subscribes
					.entry(subscribe.path.to_string())
					.or_default()
					.insert(stream, subscribe);
			}
		}
	}

	pub fn publish(&mut self, path: String, info: message::Info) {
		for (stream, announce) in &self.announces {
			if let Some(m) = announce.filter.matches(&path) {
				self.downstream
					.announce_reply(*stream, message::Announce::Active(m.capture().to_string()));
			}
		}

		self.tracks.insert(path, PublisherTrack { info });
	}

	pub fn publish_group(&mut self, track: &str, group: GroupId) {
		if let Some(subscribes) = self.subscribes.get(track) {
			for (stream, subscribe) in subscribes {
				self.downstream.subscribe_group(*stream, group);
			}
		}

		self.cache = Some(Cache {
			group,
			frames: Vec::new(),
		});
	}

	pub fn publish_frame(&mut self, track: &str, size: usize) {
		todo!()
	}

	pub fn publish_chunk(&mut self, track: &str, data: &[u8]) {
		todo!()
	}

	pub fn publish_close(&mut self, track: &str, err: Option<StreamCode>) {
		self.tracks.remove(track);

		todo!("signal closed to active subs");
	}
}

pub struct PublisherTrack {
	info: message::Info,
}

pub struct SubscriberDedup {
	upstream: Subscriber,
}

impl SubscriberDedup {
	pub fn announced(&mut self, stream: StreamId, path: Wildcard) {
		todo!()
	}

	pub fn announced_available(&mut self, stream: StreamId) -> Option<WildcardMatch> {
		todo!()
	}

	pub fn announced_unavailable(&mut self, stream: StreamId) -> Option<WildcardMatch> {
		todo!()
	}

	pub fn announced_live(&mut self, stream: StreamId) -> bool {
		todo!()
	}

	pub fn subscribe(&mut self, stream: StreamId, request: SubscribeRequest) {
		todo!()
	}

	pub fn subscribe_update(&mut self, stream: StreamId, update: SubscribeUpdate) -> Result<(), Error> {
		todo!()
	}

	pub fn subscribe_info(&mut self, stream: StreamId) -> Result<SubscribeInfo, Error> {
		todo!()
	}

	/// Returns the next group stream for the given subscription.
	pub fn subscribe_group(&mut self, stream: StreamId) -> Option<StreamId> {
		todo!()
	}

	/// Returns the size of the next frame in the group stream.
	///
	/// The caller is responsible for reading this many bytes from the stream.
	pub fn subscribe_frame(&mut self, stream: StreamId) -> Option<usize> {
		todo!()
	}
}

pub struct Relay {}
*/
