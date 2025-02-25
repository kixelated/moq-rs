use std::collections::{hash_map, BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut};

use crate::{
	coding::{Decode, DecodeError, Encode},
	message,
};

use super::{AnnounceId, Error, GroupId, Increment, Lock, StreamId, SubscribeId};

#[derive(Clone)]
pub struct Session {
	pub publisher: Publisher,
	pub subscriber: Subscriber,
	pub streams: Streams,
}

impl Session {
	pub fn new() -> Self {
		let streams = Streams::default();
		Self {
			publisher: Publisher::new(streams.clone()),
			subscriber: Subscriber::new(streams.clone()),
			streams,
		}
	}
}

#[derive(Default)]
pub struct StreamsState {
	active: HashMap<StreamId, StreamState>,

	create_bi: VecDeque<StreamKind>,
	create_uni: VecDeque<PublisherGroup>,

	encodable: BTreeSet<StreamId>,
}

#[derive(Clone, Default)]
pub struct Streams {
	state: Lock<StreamsState>,
}

impl Streams {
	pub fn encode<B: BufMut>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();
		let stream = state.active.get_mut(&id).ok_or(Error::UnknownStream)?;

		if !stream.send_buffer.is_empty() {
			let size = buf.remaining_mut().min(stream.send_buffer.len());
			buf.put_slice(&stream.send_buffer[..size]);

			// TODO This is not efficient.
			// We should use a ring buffer (VecDeque) instead, but it's not BufMut.
			stream.send_buffer.drain(..size);
		}

		let chain = &mut buf.chain_mut(&mut stream.send_buffer);
		match stream.kind {
			StreamKind::PublisherAnnounce(ref mut stream) => stream.encode(chain),
			StreamKind::PublisherSubscribe(ref mut stream) => stream.encode(chain),
			StreamKind::PublisherGroup(ref mut stream) => stream.encode(chain),
			StreamKind::SubscriberAnnounce(ref mut stream) => stream.encode(chain),
			StreamKind::SubscriberSubscribe(ref mut stream) => stream.encode(chain),
			StreamKind::SubscriberGroup(_) => unreachable!("read only"),
		};

		Ok(())
	}

	fn mark_encodable(&mut self, id: StreamId) {
		self.state.lock().encodable.insert(id);
	}

	/// Returns the next stream ID that can be newly encoded.
	pub fn encodable(&mut self) -> Option<StreamId> {
		self.state.lock().encodable.pop_first()
	}

	pub fn decode<B: Buf>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		let mut state = self.state.lock();

		let stream = match state.active.entry(id) {
			hash_map::Entry::Occupied(entry) => entry.into_mut(),
			hash_map::Entry::Vacant(entry) => {
				let kind = match id.is_bi() {
					true => match message::ControlType::decode(buf)? {
						message::ControlType::Session => todo!(),
						message::ControlType::Announce => {
							PublisherAnnounce::new(id, self.clone()).into()
						}
						message::ControlType::Subscribe => {
							PublisherSubscribe::new(self.clone()).into(),
						}
						message::ControlType::Info => todo!(),
					},
					false => match message::DataType::decode(buf)? {
						message::DataType::Group => SubscriberGroup::new(id, self.clone()).into(),
					},
				};

				entry.insert(StreamState::new(kind))
			}
		};

		let chain = &mut stream.recv_buffer.chain(buf);

		let res = match stream.kind {
			StreamKind::SubscriberAnnounce(mut stream) => stream.decode(chain),
			StreamKind::SubscriberSubscribe(mut stream) => stream.decode(chain),
			StreamKind::SubscriberGroup(mut stream) => stream.decode(chain),
			StreamKind::PublisherAnnounce(mut stream) => stream.decode(chain),
			StreamKind::PublisherSubscribe(mut stream) => stream.decode(chain),
			StreamKind::PublisherGroup(_) => unreachable!("write only"),
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

		let mut state = self.state.lock();
		let kind = match state.create_bi.pop_front() {
			None => return,
			Some(kind) => kind,
		};

		let id = id.take().unwrap();

		match kind {
			StreamKind::SubscriberAnnounce(stream) => stream.state.lock().stream = Some(id),
			StreamKind::SubscriberSubscribe(stream) => stream.state.lock().stream = Some(id),
			_ => unreachable!(),
		};

		state.encodable.insert(id);
	}

	pub fn open_uni(&mut self, stream: &mut Option<StreamId>) {
		if stream.is_none() {
			return;
		}

		let mut state = self.state.lock();
		let group = match state.create_uni.pop_front() {
			None => return,
			Some(kind) => kind,
		};

		let stream = stream.take().unwrap();
		group.state.lock().stream = Some(stream);

		state.encodable.insert(stream);
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

#[derive(Clone, derive_more::From)]
enum StreamKind {
	PublisherAnnounce(PublisherAnnounce),
	PublisherSubscribe(PublisherSubscribe),
	PublisherGroup(PublisherGroup),

	SubscriberAnnounce(SubscriberAnnounce),
	SubscriberSubscribe(SubscriberSubscribe),
	SubscriberGroup(SubscriberGroup),
}

#[derive(Default)]
struct SubscriberState {
	announced: HashMap<AnnounceId, SubscriberAnnounce>,
	announced_create: VecDeque<message::AnnouncePlease>,
	announced_next: AnnounceId,
	announced_ready: BTreeSet<SubscriberAnnounce>,

	subscribe: HashMap<SubscribeId, SubscriberSubscribe>,
	subscribe_next: SubscribeId,
	subscribe_ready: BTreeSet<SubscriberSubscribe>,

	groups: HashMap<(SubscribeId, GroupId), SubscriberGroup>,
}

#[derive(Clone)]
pub struct Subscriber {
	state: Lock<SubscriberState>,
	streams: Streams,
}

impl Subscriber {
	fn new(streams: Streams) -> Self {
		Self {
			state: Lock::default(),
			streams,
		}
	}

	pub fn create_announced(&mut self, request: message::AnnouncePlease) -> SubscriberAnnounce {
		let mut state = self.state.lock();

		let id = state.announced_next;
		let announced = SubscriberAnnounce::new(id, self.clone(), request);

		state.announced_next.increment();
		state.announced.insert(id, announced.clone());

		self.streams.state.lock().create_bi.push_back(announced.clone().into());

		announced
	}

	pub fn create_subscribe(&mut self, request: message::Subscribe) -> SubscriberSubscribe {
		let mut state = self.state.lock();

		let id = state.subscribe_next;
		let subscribe = SubscriberSubscribe::new(id, self.clone(), request);

		state.subscribe_next.increment();
		state.subscribe.insert(id, subscribe.clone());

		self.streams
			.state
			.lock()
			.create_bi
			.push_back(subscribe.clone().into());

		subscribe
	}

	/// Returns the next announcement with pending data.
	pub fn next_announced(&mut self) -> Option<SubscriberAnnounce> {
		self.state.lock().announced_ready.pop_first()
	}

	/// Returns the next subscription with pending data.
	pub fn next_subscribe(&mut self) -> Option<SubscriberSubscribe> {
		self.state.lock().subscribe_ready.pop_first()
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

#[derive(Clone)]
pub struct SubscriberAnnounce {
	id: AnnounceId,
	state: Lock<SubscriberAnnounceState>,
	subscriber: Subscriber,
}

impl SubscriberAnnounce {
	fn new(id: AnnounceId, subscriber: Subscriber, request: message::AnnouncePlease) -> Self {
		Self {
			id,
			state: Lock::new(SubscriberAnnounceState::new(request)),
			subscriber,
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

		if let Some(request) = state.request.take() {
			request.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();

		while buf.remaining() > 0 {
			let dropped = message::Announce::decode(buf)?;
			state.events.push_back(dropped);

			self.subscriber.state.lock().announced_ready.insert(self.clone());
		}

		Ok(())
	}

	pub fn event(&mut self) -> Option<message::Announce> {
		self.state.lock().events.pop_front()
	}
}

struct SubscriberSubscribeState {
	// outbound
	request: Option<message::Subscribe>,
	update: Option<message::SubscribeUpdate>,

	// inbound
	info: Option<message::Info>,
	drops: VecDeque<message::GroupDrop>,

	groups: HashMap<GroupId, SubscriberGroup>,
	groups_ready: VecDeque<SubscriberGroup>,

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
			groups: HashMap::new(),
			groups_ready: VecDeque::new(),
		}
	}
}

#[derive(Clone)]
pub struct SubscriberSubscribe {
	id: SubscribeId,
	subscriber: Subscriber,
	state: Lock<SubscriberSubscribeState>,
}

impl SubscriberSubscribe {
	fn new(id: SubscribeId, subscriber: Subscriber, request: message::Subscribe) -> Self {
		Self {
			id,
			subscriber,
			state: Lock::new(SubscriberSubscribeState::new(request)),
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

		if let Some(request) = state.request.take() {
			request.encode(buf);
		}

		if let Some(update) = state.update.take() {
			update.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();

		if state.info.is_none() {
			state.info = Some(message::Info::decode(buf)?);
			self.subscriber.state.lock().subscribe_ready.insert(self.clone());
		}

		while buf.remaining() > 0 {
			let dropped = message::GroupDrop::decode(buf)?;
			state.drops.push_back(dropped);

			self.subscriber.state.lock().subscribe_ready.insert(self.clone());
		}

		Ok(())
	}

	/// Update the subscription with a new priority/ordering.
	pub fn update(&mut self, update: message::SubscribeUpdate) {
		let mut state = self.state.lock();
		state.update = Some(update);

		if let Some(stream) = state.stream {
			self.subscriber.streams.mark_encodable(stream);
		}
	}

	/// Return information about the track if received.
	pub fn info(&mut self) -> Option<message::Info> {
		self.state.lock().info.clone()
	}

	pub fn dropped(&mut self) -> Option<message::GroupDrop> {
		self.state.lock().drops.pop_front()
	}

	/// Return the group with the given ID.
	pub fn group(&mut self, id: GroupId) -> Option<SubscriberGroup> {
		self.state.lock().groups.get(&id).cloned()
	}

	/// Returns the a group stream for the given subscription.
	pub fn group_next(&mut self) -> Option<SubscriberGroup> {
		self.state.lock().groups_ready.pop_front()
	}
}

#[derive(Default)]
pub struct SubscriberGroupState {
	frames: VecDeque<usize>,
	data: VecDeque<u8>,

	write_remain: usize,
	read_remain: usize,
}

#[derive(Clone)]
pub struct SubscriberGroup {
	id: StreamId,
	state: Lock<SubscriberGroupState>,
	subscriber: Subscriber,
}

impl SubscriberGroup {
	fn new(id: StreamId, subscriber: Subscriber) -> Self {
		Self {
			id,
			state: Lock::new(SubscriberGroupState::default()),
			subscriber,
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();

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
		let mut state = self.state.lock();
		if state.read_remain == 0 {
			state.read_remain = state.frames.pop_front()?;
		}

		Some(state.read_remain)
	}

	/// Read the next chunk of the frame if available.
	pub fn read<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

		let size = buf.remaining_mut().min(state.read_remain);
		buf.limit(size).put(&mut state.data);
	}
}

#[derive(Default)]
pub struct PublisherState {
	announces: HashMap<AnnounceId, PublisherAnnounce>,
	announced_ready: BTreeSet<PublisherAnnounce>,

	subscribes: HashMap<SubscribeId, PublisherSubscribe>,
	subscribe_ready: BTreeSet<PublisherSubscribe>,

	groups: HashMap<(SubscribeId, GroupId), PublisherGroup>,
}

#[derive(Clone)]
pub struct Publisher {
	state: Lock<PublisherState>,
	streams: Streams,
}

impl Publisher {
	fn new(streams: Streams) -> Self {
		Self {
			state: Lock::default(),
			streams,
		}
	}
}

#[derive(Default)]
struct PublisherAnnounceState {
	request: Option<message::AnnouncePlease>,
	events: VecDeque<message::Announce>,
}

impl PublisherAnnounceState {
	pub fn new(stream: StreamId) -> Self {
		Self {
			request: None,
			events: VecDeque::new(),
		}
	}
}

#[derive(Clone)]
pub struct PublisherAnnounce {
	id: StreamId,
	state: Lock<PublisherAnnounceState>,
	publisher: Publisher,
}

impl PublisherAnnounce {
	fn new(id: StreamId, publisher: Publisher) -> Self {
		Self {
			id,
			state: Lock::new(PublisherAnnounceState::default()),
			publisher,
		}
	}

	pub fn request(&self) -> message::AnnouncePlease {
		self.state.lock().request.as_ref().unwrap().clone()
	}

	pub fn reply(&mut self, msg: message::Announce) {
		let mut state = self.state.lock();
		state.events.push_back(msg);
		self.publisher.streams.mark_encodable(self.id);
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();
		state.events.push_back(message::Announce::decode(buf)?);

		self.publisher.streams.mark_encodable(self.id);

		Ok(())
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

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

#[derive(Clone)]
pub struct PublisherSubscribe {
	state: Lock<PublisherSubscribeState>,
	publisher: Publisher,
}

impl PublisherSubscribe {
	pub fn request(&self) -> message::Subscribe {
		let state = self.state.lock();
		state.request.clone().unwrap()
	}

	pub fn info(&mut self, info: message::Info) {
		let mut state = self.state.lock();
		assert!(!state.info_sent); // TODO return an error instead
		state.info = Some(info);
		state.info_sent = true;

		self.publisher.streams.mark_encodable(state.stream);
	}

	pub fn dropped(&mut self, dropped: message::GroupDrop) {
		let mut state = self.state.lock();
		state.dropped.push_back(dropped);

		self.publisher.streams.encodable.insert(state.stream);
	}

	pub fn create_group(&mut self, group: GroupId) -> PublisherGroup {
		let group = PublisherGroup::new(self.publisher.clone());

		self.session.publisher.groups.insert(id, PublisherGroupState::default());
		self.session.streams.create_bi.push_back(StreamKind::PublisherGroup(id));

		PublisherGroup {
			session: self.session,
			id,
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

		if let Some(info) = state.info.take() {
			info.encode(buf);
		}

		while let Some(dropped) = state.dropped.pop_front() {
			dropped.encode(buf);
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let mut state = self.state.lock();

		if state.request.is_none() {
			state.request = Some(message::Subscribe::decode(buf)?);
			self.publisher.state.lock().subscribe_ready.insert(self.clone());
		}

		while buf.has_remaining() {
			let update = message::SubscribeUpdate::decode(buf)?;
			state.update = Some(update);
			self.publisher.state.lock().subscribe_ready.insert(self.clone());
		}

		Ok(())
	}
}

#[derive(Default)]
struct PublisherGroupState {
	stream: Option<StreamId>,
	frames: VecDeque<usize>,
	remain: usize, // remaining in the current frame.
	buffer: VecDeque<u8>,
}

#[derive(Clone)]
pub struct PublisherGroup {
	state: Lock<PublisherGroupState>,
	publisher: Publisher,
}

impl PublisherGroup {
	fn new(publisher: Publisher) -> Self {
		Self {
			state: Lock::new(PublisherGroupState::default()),
			publisher,
		}
	}

	/// Start a new frame with the indicated size.
	pub fn frame(&mut self, size: usize) {
		let mut state = self.state.lock();
		state.frames.push_back(size);

		if let Some(stream) = state.stream {
			self.publisher.streams.mark_encodable(stream);
		}
	}

	pub fn write<B: Buf>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();
		while buf.has_remaining() {
			let chunk = buf.chunk();
			state.buffer.extend(chunk);
			buf.advance(chunk.len());
		}

		if let Some(stream) = state.stream {
			self.publisher.streams.mark_encodable(stream);
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		let mut state = self.state.lock();

		if state.remain == 0 {
			let size = match state.frames.pop_front() {
				Some(size) => size,
				None => return,
			};

			state.remain = size;

			message::Frame { size }.encode(buf);
		}

		let size = buf.remaining_mut().min(state.remain).min(state.buffer.len());
		let parts = state.buffer.as_slices();

		buf.put_slice(&parts.0[..size.min(parts.0.len())]);
		buf.put_slice(&parts.1[..(size.saturating_sub(parts.0.len()))]);

		state.buffer.drain(..size);
		state.remain -= size;
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
