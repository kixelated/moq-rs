use std::collections::{BTreeSet, HashMap, VecDeque};

use bytes::{Buf, BufMut, Bytes, BytesMut};
use derive_more::From;

use crate::{
	coding::{Decode, DecodeError, Encode},
	message::{self, GroupOrder},
};

use super::{
	AnnounceId, Error, ErrorCode, GroupId, Increment, PublisherEvent, PublisherState, SessionEvent, SessionState,
	StreamEvent, StreamId, StreamKind, StreamsState, SubscribeId, SubscriberEvent, SubscriberState,
};

#[derive(Debug, From)]
pub enum ConnectionEvent {
	Session(SessionEvent),
	Stream(StreamEvent),
	Publisher(PublisherEvent),
	Subscriber(SubscriberEvent),
}

pub struct Connection {
	session: SessionState,
	publisher: PublisherState,
	subscriber: SubscriberState,
	streams: StreamsState,
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

		this.streams.create(StreamKind::Session);

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

	pub fn poll(&mut self) -> Option<ConnectionEvent> {
		todo!()
	}
}

pub struct Streams<'a> {
	connection: &'a mut Connection,
}

impl<'a> Streams<'a> {
	pub fn encode<B: BufMut>(&mut self, id: StreamId, buf: &mut B) {
		let stream = self.connection.streams.get_mut(id).unwrap();

		// Use any data already in the buffer.
		if !stream.send_buffer.is_empty() {
			buf.put(&mut stream.send_buffer);
			return;
		}

		let mut overflow = BytesMut::new();
		let chain = &mut buf.chain_mut(&mut overflow);

		match stream.kind {
			StreamKind::Session => self.connection.session.encode(chain),
			StreamKind::Publisher(kind) => self.connection.publisher.encode(kind, chain),
			StreamKind::Subscriber(kind) => self.connection.subscriber.encode(kind, chain),
			StreamKind::Unknown(_) => unreachable!("unknown type"),
		};

		stream.send_buffer = overflow.freeze();
	}

	pub fn decode(&mut self, id: StreamId, mut buf: &[u8]) -> Result<(), Error> {
		if !buf.has_remaining() {
			return Ok(());
		}

		let stream = self.connection.streams.get_or_create(id);

		// Chain the Buf, so we'll decode the old data first then the new data.
		let chain = &mut stream.recv_buffer.chain(&mut buf);

		while chain.has_remaining() {
			match Self::recv(
				&mut stream.kind,
				chain,
				&mut self.connection.session,
				&mut self.connection.publisher,
				&mut self.connection.subscriber,
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

	pub fn open(&mut self, id: StreamId, kind: StreamKind) {
		match kind {
			StreamKind::Subscriber(kind) => self.connection.subscriber.open(id, kind),
			StreamKind::Publisher(kind) => self.connection.publisher.open(id, kind),
			StreamKind::Session => self.connection.session.open(id),
			_ => unreachable!(),
		};

		self.connection.streams.encodable(id);
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
			StreamKind::Unknown(stream) => {
				*kind = if stream.is_uni() {
					match message::DataType::decode(buf)? {
						message::DataType::Group => StreamKind::Subscriber(subscriber.accept_group(stream, buf)?),
					}
				} else {
					match message::ControlType::decode(buf)? {
						message::ControlType::Session => {
							session.accept(stream, buf)?;
							StreamKind::Session
						}
						message::ControlType::Announce => {
							StreamKind::Publisher(publisher.accept_announce(stream, buf)?)
						}
						message::ControlType::Subscribe => {
							StreamKind::Publisher(publisher.accept_subscribe(stream, buf)?)
						}
						message::ControlType::Info => todo!(),
					}
				}
			}
			StreamKind::Session => session.decode(buf)?,
			StreamKind::Publisher(kind) => publisher.decode(kind, buf)?,
			StreamKind::Subscriber(kind) => subscriber.decode(kind, buf)?,
		}

		Ok(())
	}
}

pub struct Publisher<'a> {
	connection: &'a mut Connection,
}

impl<'a> Publisher<'a> {
	pub fn announce(&mut self) -> PublisherAnnounce {
		PublisherAnnounce {
			connection: self.connection,
		}
	}

	pub fn subscribed(&mut self) -> PublisherSubscribe {
		PublisherSubscribe {
			connection: self.connection,
		}
	}

	pub fn groups(&mut self) -> PublisherGroups {
		PublisherGroups {
			connection: self.connection,
		}
	}
}

pub struct PublisherAnnounce<'a> {
	connection: &'a mut Connection,
}

impl<'a> PublisherAnnounce<'a> {
	pub fn accept(&mut self) -> Option<AnnounceId> {
		self.connection.publisher.announced_ready.pop_first()
	}

	pub fn requested(&mut self, id: AnnounceId) -> &message::AnnouncePlease {
		&self.connection.publisher.announces.get(&id).unwrap().request
	}

	pub fn reply(&mut self, id: AnnounceId, msg: message::Announce) {
		let announce = self.connection.publisher.announces.get_mut(&id).unwrap();
		announce.events.push_back(msg);

		self.connection.streams.encodable.insert(announce.stream);
	}
}

pub struct PublisherSubscribe<'a> {
	connection: &'a mut Connection,
}

impl<'a> PublisherSubscribe<'a> {
	pub fn accept(&mut self) -> Option<SubscribeId> {
		self.connection.publisher.subscribe_ready.pop_first()
	}

	pub fn requested(&mut self, id: SubscribeId) -> &SubscribeRequest {
		&self.connection.publisher.subscribes.get(&id).unwrap().request
	}

	pub fn respond(&mut self, id: SubscribeId, info: message::Info) {
		let sub = self.connection.publisher.subscribes.get_mut(&id).unwrap();
		assert!(sub.info.is_none());
		assert!(!sub.info_sent);

		sub.info = Some(info);

		self.connection.streams.encodable.insert(sub.stream);
	}
}

pub struct PublisherGroups<'a> {
	connection: &'a mut Connection,
}

impl PublisherGroups<'_> {
	pub fn start(&mut self, id: SubscribeId, group: GroupId) {
		self.connection
			.publisher
			.groups
			.insert((id, group), PublisherGroupState::default());
		self.connection
			.streams
			.create
			.push_back(StreamKind::PublisherGroup((id, group)));
	}

	/// Write an entire frame to the stream.
	pub fn frame<B: Buf>(&mut self, id: SubscribeId, group: GroupId, data: &mut B) {
		let group = self.connection.publisher.groups.get_mut(&(id, group)).unwrap();
		// NOTE: This does not make a copy if data is already a Bytes.
		let mut data = data.copy_to_bytes(data.remaining());
		group.frame(data.len());
		group.write(&mut data);
	}

	/// Mark the start of a new frame with the given size.
	///
	/// WARN: This will panic if the previous frame was not fully written.
	pub fn frame_size(&mut self, id: SubscribeId, group: GroupId, size: usize) {
		let group = self.connection.publisher.groups.get_mut(&(id, group)).unwrap();
		group.frame(size);
	}

	/// Write a chunk of the frame, which MUST be preceded by a call to [Self::frame_size].
	///
	/// WARN: This will panic if you write more than promised via [Self::frame_size].
	pub fn frame_chunk<B: Buf>(&mut self, id: SubscribeId, group: GroupId, chunk: &mut B) {
		let group = self.connection.publisher.groups.get_mut(&(id, group)).unwrap();
		group.write(chunk);

		if let Some(stream) = group.stream {
			self.connection.streams.encodable.insert(stream);
		}
	}

	pub fn close(&mut self, id: SubscribeId, group: GroupId, error: Option<ErrorCode>) {
		let sub = self.connection.publisher.subscribes.get_mut(&id).unwrap();

		if let Some(error) = error {
			sub.dropped.push_back((group, error));
		}

		self.connection.streams.encodable.insert(sub.stream);
		todo!("clean close")
	}
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
			.create
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
			.create
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
