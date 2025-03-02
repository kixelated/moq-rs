use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes};

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self, GroupOrder},
};

use super::{AnnounceId, Error, ErrorCode, GroupId, Increment, StreamId, SubscribeId};

pub struct Connection {
	session: SessionState,
	publisher: PublisherState,
	subscriber: SubscriberState,
	streams: StreamsState,
}

#[derive(Clone, Debug)]
enum SessionState {
	ClientInit,
	ServerInit,
	ClientSent(message::ClientSetup),
	ServerSend(message::ClientSetup, message::ServerSetup),
	Connected(message::ClientSetup, message::ServerSetup),
}

/* 	let client = message::ClientSetup {
	versions: [message::Version::CURRENT].into(),
	extensions: Default::default(),
};

setup.writer.encode(&client).await?;
let server: message::ServerSetup = setup.reader.decode().await?;

tracing::info!(version = ?server.version, "connected"); */

impl SessionState {
	fn new(client: bool) -> Self {
		match client {
			true => Self::ClientInit,
			false => Self::ServerInit,
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		match self {
			Self::ClientInit => {
				message::ControlType::Session.encode(buf);

				let client = message::ClientSetup {
					versions: [message::Version::CURRENT].into(),
					extensions: Default::default(),
				};
				client.encode(buf);

				*self = Self::ClientSent(client);
			}
			Self::ServerSend(client, server) => {
				server.encode(buf);
				*self = Self::Connected(client.clone(), server.clone());
			}
			Self::ServerInit | Self::ClientSent(_) | Self::Connected(_, _) => {}
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		match self {
			Self::ServerInit => {
				let client = message::ClientSetup::decode(buf)?;
				if !client.versions.contains(&message::Version::CURRENT) {
					todo!("version error");
				}

				let server = message::ServerSetup {
					version: message::Version::CURRENT,
					extensions: Default::default(),
				};

				*self = Self::ServerSend(client, server);
			}
			Self::ClientSent(client) => {
				let server = message::ServerSetup::decode(buf)?;
				if server.version != message::Version::CURRENT {
					todo!("version error");
				}

				*self = Self::Connected(client.clone(), server);
			}
			Self::Connected(_, _) => {
				let _info = message::SessionInfo::decode(buf)?;
				// TODO use info
			}
			Self::ClientInit | Self::ServerSend(_, _) => todo!("unexpected stream error"),
		}

		Ok(())
	}
}

#[derive(Default)]
struct StreamsState {
	active: HashMap<StreamId, Stream>,

	create_bi: VecDeque<StreamKind>,
	create_uni: VecDeque<(SubscribeId, GroupId)>,

	encodable: BTreeSet<StreamId>,
}

impl Connection {
	// Create a new client connection.
	pub fn client() -> Self {
		let mut this = Self {
			session: SessionState::new(true),
			publisher: PublisherState::default(),
			subscriber: SubscriberState::default(),
			streams: StreamsState::default(),
		};

		this.streams.create_bi.push_back(StreamKind::Session);

		this
	}

	// Create a new server connection.
	pub fn server() -> Self {
		Self {
			session: SessionState::new(false),
			publisher: PublisherState::default(),
			subscriber: SubscriberState::default(),
			streams: StreamsState::default(),
		}
	}

	pub fn connected(&mut self) -> Result<bool, Error> {
		match &mut self.session {
			SessionState::Connected(_, _) => Ok(true),
			_ => Ok(false),
		}
	}

	pub fn encode<B: BufMut>(&mut self, id: StreamId, buf: &mut B) -> Result<(), Error> {
		let stream = self.streams.active.get_mut(&id).ok_or(Error::UnknownStream)?;

		if !stream.send_buffer.is_empty() {
			let size = buf.remaining_mut().min(stream.send_buffer.len());
			buf.put_slice(&stream.send_buffer[..size]);

			// TODO This is not efficient.
			// We should use a ring buffer (VecDeque) instead, but it's not BufMut.
			stream.send_buffer.drain(..size);
		}

		let chain = &mut buf.chain_mut(&mut stream.send_buffer);
		match stream.kind {
			StreamKind::Session => self.session.encode(chain),
			StreamKind::PublisherAnnounce(id) => self.publisher.announces.get_mut(&id).unwrap().encode(chain),
			StreamKind::PublisherSubscribe(id) => self.publisher.subscribes.get_mut(&id).unwrap().encode(chain),
			StreamKind::PublisherGroup(id) => self.publisher.groups.get_mut(&id).unwrap().encode(chain),
			StreamKind::SubscriberAnnounce(id) => self.subscriber.announced.get_mut(&id).unwrap().encode(chain),
			StreamKind::SubscriberSubscribe(id) => self.subscriber.subscribe.get_mut(&id).unwrap().encode(chain),
			StreamKind::SubscriberGroup(_) => unreachable!("read only"),
			StreamKind::RecvStream(_) => unreachable!("unknown type"),
		};

		Ok(())
	}

	/// Returns the next stream ID that can be newly encoded.
	pub fn encodable(&mut self) -> Option<StreamId> {
		self.streams.encodable.pop_first()
	}

	pub fn decode(&mut self, id: StreamId, mut buf: &[u8]) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		let stream = self
			.streams
			.active
			.entry(id)
			.or_insert_with(|| Stream::new(StreamKind::RecvStream(id)));

		// Chain the Buf, so we'll decode the old data first then the new data.
		let chain = &mut stream.recv_buffer.chain(&mut buf);

		while chain.has_remaining() {
			match Self::recv(
				&mut stream.kind,
				chain,
				&mut self.session,
				&mut self.publisher,
				&mut self.subscriber,
			) {
				Ok(()) => continue,
				Err(Error::Coding(DecodeError::Short)) => {
					// We need to keep the buffer for the next call.
					// Put the remainder of the buffer back.
					stream.recv_buffer.put(buf);
					return Ok(());
				}
				Err(err) => return Err(err),
			}
		}

		Ok(())
	}

	pub fn open_bi(&mut self, id: &mut Option<StreamId>) {
		if id.is_none() {
			return;
		}

		let kind = match self.streams.create_bi.pop_front() {
			None => return,
			Some(kind) => kind,
		};

		let id = id.take().unwrap();

		match kind {
			StreamKind::SubscriberAnnounce(announce) => {
				self.subscriber.announced.get_mut(&announce).unwrap().stream = Some(id)
			}
			StreamKind::SubscriberSubscribe(subscribe) => {
				self.subscriber.subscribe.get_mut(&subscribe).unwrap().stream = Some(id)
			}
			_ => unreachable!(),
		};

		self.streams.encodable.insert(id);
	}

	pub fn open_uni(&mut self, stream: &mut Option<StreamId>) {
		if stream.is_none() {
			return;
		}

		loop {
			let id = match self.streams.create_uni.pop_front() {
				None => return,
				Some(kind) => kind,
			};

			let group = match self.publisher.groups.get_mut(&id) {
				None => continue,
				Some(group) => group,
			};

			let stream = stream.take().unwrap();
			group.stream = Some(stream);

			self.streams.encodable.insert(stream);
			return;
		}
	}

	// Partially decode a stream, with the remainder (on error) being put back into the buffer.
	// This doesn't take self because StreamsState is partially borrowed.
	fn recv<B: Buf>(
		kind: &mut StreamKind,
		buf: &mut B,
		session: &mut SessionState,
		publisher: &mut PublisherState,
		subscriber: &mut SubscriberState,
	) -> Result<(), Error> {
		match *kind {
			StreamKind::RecvStream(stream) => {
				*kind = if stream.is_uni() {
					match message::DataType::decode(buf)? {
						message::DataType::Group => Self::recv_group(buf, subscriber)?,
					}
				} else {
					match message::ControlType::decode(buf)? {
						message::ControlType::Session => Self::recv_session(buf, session)?,
						message::ControlType::Announce => Self::recv_announce(buf, publisher, stream)?,
						message::ControlType::Subscribe => Self::recv_subscribe(buf, publisher, stream)?,
						message::ControlType::Info => todo!(),
					}
				}
			}
			StreamKind::Session => session.decode(buf)?,
			StreamKind::PublisherAnnounce(id) => {
				publisher.announces.get_mut(&id).unwrap().decode(buf)?;
				publisher.announced_ready.insert(id);
			}
			StreamKind::PublisherSubscribe(id) => {
				publisher.subscribes.get_mut(&id).unwrap().decode(buf)?;
				publisher.subscribe_ready.insert(id);
			}
			StreamKind::PublisherGroup(_) => unreachable!("write only"),
			StreamKind::SubscriberAnnounce(id) => {
				subscriber.announced.get_mut(&id).unwrap().decode(buf)?;
				subscriber.announced_ready.insert(id);
			}
			StreamKind::SubscriberSubscribe(id) => {
				subscriber.subscribe.get_mut(&id).unwrap().decode(buf)?;
				subscriber.subscribe_ready.insert(id);
			}
			StreamKind::SubscriberGroup(id) => {
				subscriber.groups.get_mut(&id).unwrap().decode(buf)?;
				subscriber.groups_ready.insert(id);
			}
		}

		Ok(())
	}

	fn recv_session<B: Buf>(buf: &mut B, session: &mut SessionState) -> Result<StreamKind, Error> {
		session.decode(buf)?;
		Ok(StreamKind::Session)
	}

	fn recv_group<B: Buf>(buf: &mut B, subscriber: &mut SubscriberState) -> Result<StreamKind, Error> {
		let group = message::Group::decode(buf)?;
		let id = (group.subscribe.into(), group.sequence.into());

		let group = SubscriberGroupState::default();
		subscriber.groups.insert(id, group);
		subscriber.groups_ready.insert(id);

		Ok(StreamKind::SubscriberGroup(id))
	}

	fn recv_announce<B: Buf>(
		buf: &mut B,
		publisher: &mut PublisherState,
		stream: StreamId,
	) -> Result<StreamKind, Error> {
		let announce = message::AnnouncePlease::decode(buf)?;
		let id = publisher.announced_next;
		publisher.announced_next.increment();

		let announce = PublisherAnnounceState::new(stream, announce);
		publisher.announces.insert(id, announce);
		publisher.announced_ready.insert(id);

		Ok(StreamKind::PublisherAnnounce(id))
	}

	fn recv_subscribe<B: Buf>(
		buf: &mut B,
		publisher: &mut PublisherState,
		stream: StreamId,
	) -> Result<StreamKind, Error> {
		let subscribe = message::Subscribe::decode(buf)?;
		let id = publisher.subscribe_next;
		publisher.subscribe_next.increment();

		let subscribe = PublisherSubscribeState::new(stream, subscribe.into());
		publisher.subscribes.insert(id, subscribe);
		publisher.subscribe_ready.insert(id);

		Ok(StreamKind::PublisherSubscribe(id))
	}
}

struct Stream {
	kind: StreamKind,
	send_buffer: Vec<u8>,
	recv_buffer: Vec<u8>,
}

impl Stream {
	pub fn new(kind: StreamKind) -> Self {
		Self {
			kind,
			send_buffer: Vec::new(),
			recv_buffer: Vec::new(),
		}
	}
}

#[derive(Clone)]
enum StreamKind {
	Session,

	RecvStream(StreamId),

	PublisherAnnounce(AnnounceId),
	PublisherSubscribe(SubscribeId),
	PublisherGroup((SubscribeId, GroupId)),

	SubscriberAnnounce(AnnounceId),
	SubscriberSubscribe(SubscribeId),
	SubscriberGroup((SubscribeId, GroupId)),
}

#[derive(Default)]
struct SubscriberState {
	announced: HashMap<AnnounceId, SubscriberAnnounceState>,
	announced_next: AnnounceId,
	announced_ready: BTreeSet<AnnounceId>,

	subscribe: HashMap<SubscribeId, SubscriberSubscribeState>,
	subscribe_next: SubscribeId,
	subscribe_ready: BTreeSet<SubscribeId>,

	groups: HashMap<(SubscribeId, GroupId), SubscriberGroupState>,
	groups_ready: BTreeSet<(SubscribeId, GroupId)>,
}

pub struct Subscriber<'a> {
	session: &'a mut Connection,
}

impl<'a> Subscriber<'a> {
	pub fn announced(&mut self) -> SubscriberAnnounced {
		SubscriberAnnounced { session: self.session }
	}

	pub fn subscribes(&mut self) -> SubscriberSubscribe {
		SubscriberSubscribe { session: self.session }
	}
}

pub struct SubscriberAnnounced<'a> {
	session: &'a mut Connection,
}

impl<'a> SubscriberAnnounced<'a> {
	pub fn start(&mut self, request: message::AnnouncePlease) -> AnnounceId {
		let id = self.session.subscriber.announced_next;
		let announced = SubscriberAnnounceState::new(request);

		self.session.subscriber.announced_next.increment();
		self.session.subscriber.announced.insert(id, announced);

		self.session
			.streams
			.create_bi
			.push_back(StreamKind::SubscriberAnnounce(id));

		id
	}

	/// Returns the next announcement with pending data.
	pub fn ready(&mut self) -> Option<AnnounceId> {
		self.session.subscriber.announced_ready.pop_first()
	}

	pub fn event(&mut self, id: AnnounceId) -> Option<message::Announce> {
		self.session.subscriber.announced.get_mut(&id)?.events.pop_front()
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

pub struct SubscriberSubscribe<'a> {
	session: &'a mut Connection,
}

impl<'a> SubscriberSubscribe<'a> {
	pub fn start(&mut self, request: SubscribeRequest) -> SubscribeId {
		let id = self.session.subscriber.subscribe_next;

		let msg = request.into_message(id.0);
		let subscribe = SubscriberSubscribeState::new(msg);

		self.session.subscriber.subscribe_next.increment();
		self.session.subscriber.subscribe.insert(id, subscribe);

		self.session
			.streams
			.create_bi
			.push_back(StreamKind::SubscriberSubscribe(id));

		id
	}

	/// Returns the next subscription with pending data.
	pub fn ready(&mut self) -> Option<SubscribeId> {
		todo!()
	}

	/// Update the subscription with a new priority/ordering.
	pub fn update(&mut self, id: SubscribeId, update: message::SubscribeUpdate) {
		let sub = self.session.subscriber.subscribe.get_mut(&id).unwrap();
		sub.update = Some(update);

		if let Some(stream) = sub.stream {
			self.session.streams.encodable.insert(stream);
		}
	}

	/// Return information about the track if received.
	pub fn info(&mut self, id: SubscribeId) -> Option<&message::Info> {
		self.session.subscriber.subscribe.get(&id)?.info.as_ref()
	}

	pub fn dropped(&mut self, id: SubscribeId) -> Option<message::GroupDrop> {
		self.session.subscriber.subscribe.get_mut(&id)?.drops.pop_front()
	}

	/// Returns the next group with pending data for the given subscription.
	pub fn group_ready(&mut self, id: SubscribeId) -> Option<GroupId> {
		self.session.subscriber.subscribe.get_mut(&id)?.groups_ready.pop_front()
	}

	/// Returns the remaining size of the group.
	pub fn group_read<B: BufMut>(&mut self, id: SubscribeId, group: GroupId, buf: &mut B) -> Option<usize> {
		let state = self.session.subscriber.groups.get_mut(&(id, group))?;
		state.read(buf)
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

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
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
	fn new(request: message::Subscribe) -> Self {
		Self {
			stream: None,
			request: Some(request),
			update: None,

			info: None,
			drops: VecDeque::new(),
			groups_ready: VecDeque::new(),
		}
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
		if let Some(request) = self.request.take() {
			request.encode(buf);
		}

		if let Some(update) = self.update.take() {
			update.encode(buf);
		}
	}

	/// Decode the next frame from the stream.
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
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
	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
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

#[derive(Default)]
pub struct PublisherState {
	announces: HashMap<AnnounceId, PublisherAnnounceState>,
	announced_ready: BTreeSet<AnnounceId>,
	announced_next: AnnounceId,

	subscribes: HashMap<SubscribeId, PublisherSubscribeState>,
	subscribe_ready: BTreeSet<SubscribeId>,
	subscribe_next: SubscribeId,

	groups: HashMap<(SubscribeId, GroupId), PublisherGroupState>,
}

pub struct Publisher<'a> {
	session: &'a mut Connection,
}

impl<'a> Publisher<'a> {
	pub fn announce(&mut self) -> PublisherAnnounce {
		PublisherAnnounce { session: self.session }
	}

	pub fn subscribed(&mut self) -> PublisherSubscribe {
		PublisherSubscribe { session: self.session }
	}

	pub fn groups(&mut self) -> PublisherGroups {
		PublisherGroups { session: self.session }
	}
}

pub struct PublisherAnnounce<'a> {
	session: &'a mut Connection,
}

impl<'a> PublisherAnnounce<'a> {
	pub fn accept(&mut self) -> Option<AnnounceId> {
		self.session.publisher.announced_ready.pop_first()
	}

	pub fn requested(&mut self, id: AnnounceId) -> &message::AnnouncePlease {
		&self.session.publisher.announces.get(&id).unwrap().request
	}

	pub fn reply(&mut self, id: AnnounceId, msg: message::Announce) {
		let announce = self.session.publisher.announces.get_mut(&id).unwrap();
		announce.events.push_back(msg);

		self.session.streams.encodable.insert(announce.stream);
	}
}

pub struct PublisherSubscribe<'a> {
	session: &'a mut Connection,
}

impl<'a> PublisherSubscribe<'a> {
	pub fn accept(&mut self) -> Option<SubscribeId> {
		self.session.publisher.subscribe_ready.pop_first()
	}

	pub fn requested(&mut self, id: SubscribeId) -> &SubscribeRequest {
		&self.session.publisher.subscribes.get(&id).unwrap().request
	}

	pub fn respond(&mut self, id: SubscribeId, info: message::Info) {
		let sub = self.session.publisher.subscribes.get_mut(&id).unwrap();
		assert!(sub.info.is_none());
		assert!(!sub.info_sent);

		sub.info = Some(info);

		self.session.streams.encodable.insert(sub.stream);
	}
}

pub struct PublisherGroups<'a> {
	session: &'a mut Connection,
}

impl PublisherGroups<'_> {
	pub fn start(&mut self, id: SubscribeId, group: GroupId) {
		self.session
			.publisher
			.groups
			.insert((id, group), PublisherGroupState::default());
		self.session
			.streams
			.create_bi
			.push_back(StreamKind::PublisherGroup((id, group)));
	}

	/// Write an entire frame to the stream.
	pub fn frame<B: Buf>(&mut self, id: SubscribeId, group: GroupId, data: &mut B) {
		let group = self.session.publisher.groups.get_mut(&(id, group)).unwrap();
		// NOTE: This does not make a copy if data is already a Bytes.
		let mut data = data.copy_to_bytes(data.remaining());
		group.frame(data.len());
		group.write(&mut data);
	}

	/// Mark the start of a new frame with the given size.
	///
	/// WARN: This will panic if the previous frame was not fully written.
	pub fn frame_size(&mut self, id: SubscribeId, group: GroupId, size: usize) {
		let group = self.session.publisher.groups.get_mut(&(id, group)).unwrap();
		group.frame(size);
	}

	/// Write a chunk of the frame, which MUST be preceded by a call to [Self::frame_size].
	///
	/// WARN: This will panic if you write more than promised via [Self::frame_size].
	pub fn frame_chunk<B: Buf>(&mut self, id: SubscribeId, group: GroupId, chunk: &mut B) {
		let group = self.session.publisher.groups.get_mut(&(id, group)).unwrap();
		group.write(chunk);

		if let Some(stream) = group.stream {
			self.session.streams.encodable.insert(stream);
		}
	}

	pub fn close(&mut self, id: SubscribeId, group: GroupId, error: Option<ErrorCode>) {
		let sub = self.session.publisher.subscribes.get_mut(&id).unwrap();

		if let Some(error) = error {
			sub.dropped.push_back((group, error));
		}

		self.session.streams.encodable.insert(sub.stream);
		todo!("clean close")
	}
}

struct PublisherAnnounceState {
	stream: StreamId,
	request: message::AnnouncePlease,
	events: VecDeque<message::Announce>,
}

impl PublisherAnnounceState {
	fn new(stream: StreamId, request: message::AnnouncePlease) -> Self {
		Self {
			stream,
			request,
			events: VecDeque::new(),
		}
	}

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
		let msg = message::Announce::decode(buf)?;
		self.events.push_back(msg);

		Ok(())
	}

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
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

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
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

	fn decode<B: Buf>(&mut self, buf: &mut B) -> Result<(), Error> {
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

	fn encode<B: BufMut>(&mut self, buf: &mut B) {
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
