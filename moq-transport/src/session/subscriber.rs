use std::{
	collections::{hash_map, HashMap},
	io,
	sync::{atomic, Arc, Mutex},
};

use crate::{
	coding::{Decode, Reader},
	data, message, serve, setup,
	util::Queue,
};

use webtransport_generic::RecvStream;

use super::{Announced, AnnouncedRecv, Session, SessionError, Subscribe};

// TODO remove Clone.
#[derive(Clone)]
pub struct Subscriber<S: webtransport_generic::Session> {
	announced: Arc<Mutex<HashMap<String, AnnouncedRecv<S>>>>,
	announced_queue: Queue<Announced<S>, SessionError>,

	subscribes: Arc<Mutex<HashMap<u64, Subscribe<S>>>>,
	subscribe_next: Arc<atomic::AtomicU64>,

	outgoing: Queue<message::Message, SessionError>,
}

impl<S: webtransport_generic::Session> Subscriber<S> {
	pub(super) fn new(outgoing: Queue<message::Message, SessionError>) -> Self {
		Self {
			announced: Default::default(),
			announced_queue: Default::default(),
			subscribes: Default::default(),
			subscribe_next: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: S) -> Result<(Session<S>, Self), SessionError> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: S) -> Result<(Session<S>, Self), SessionError> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Result<Announced<S>, SessionError> {
		self.announced_queue.pop().await
	}

	pub fn subscribe(&mut self, track: serve::TrackWriter) -> Result<(), SessionError> {
		let id = self.subscribe_next.fetch_add(1, atomic::Ordering::Relaxed);

		let subscribe = Subscribe::new(self.clone(), id, track)?;
		self.subscribes.lock().unwrap().insert(id, subscribe);

		Ok(())
	}

	pub(super) fn send_message<M: Into<message::Subscriber>>(&mut self, msg: M) -> Result<(), SessionError> {
		let msg = msg.into();

		// Remove our entry on terminal state.
		match &msg {
			message::Subscriber::AnnounceCancel(msg) => self.drop_announce(&msg.namespace),
			message::Subscriber::AnnounceError(msg) => self.drop_announce(&msg.namespace),
			// TODO not terminal; we should wait for the done message.
			message::Subscriber::Unsubscribe(msg) => self.drop_subscribe(msg.id),
			_ => {}
		}

		log::debug!("sending message: {:?}", msg);
		self.outgoing.push(msg.into())
	}

	pub(super) fn recv_message(&mut self, msg: message::Publisher) -> Result<(), SessionError> {
		log::debug!("received message: {:?}", msg);

		match msg {
			message::Publisher::Announce(msg) => self.recv_announce(msg),
			message::Publisher::Unannounce(msg) => self.recv_unannounce(msg),
			message::Publisher::SubscribeOk(msg) => self.recv_subscribe_ok(msg),
			message::Publisher::SubscribeError(msg) => self.recv_subscribe_error(msg),
			message::Publisher::SubscribeDone(msg) => self.recv_subscribe_done(msg),
		}
	}

	fn recv_announce(&mut self, msg: message::Announce) -> Result<(), SessionError> {
		let mut announces = self.announced.lock().unwrap();

		let entry = match announces.entry(msg.namespace.clone()) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let (announced, recv) = Announced::new(self.clone(), msg.namespace);
		self.announced_queue.push(announced)?;
		entry.insert(recv);

		Ok(())
	}

	fn recv_unannounce(&mut self, msg: message::Unannounce) -> Result<(), SessionError> {
		if let Some(mut announce) = self.announced.lock().unwrap().remove(&msg.namespace) {
			announce.recv_unannounce().ok();
		}

		Ok(())
	}

	fn recv_subscribe_ok(&mut self, msg: message::SubscribeOk) -> Result<(), SessionError> {
		if let Some(sub) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			sub.recv_ok(msg).ok();
		}

		Ok(())
	}

	fn recv_subscribe_error(&mut self, msg: message::SubscribeError) -> Result<(), SessionError> {
		if let Some(mut subscriber) = self.subscribes.lock().unwrap().remove(&msg.id) {
			subscriber.recv_error(msg.code).ok();
		}

		Ok(())
	}

	fn recv_subscribe_done(&mut self, msg: message::SubscribeDone) -> Result<(), SessionError> {
		if let Some(mut subscriber) = self.subscribes.lock().unwrap().remove(&msg.id) {
			subscriber.recv_done(msg.code).ok();
		}

		Ok(())
	}

	fn drop_announce(&mut self, namespace: &str) {
		self.announced.lock().unwrap().remove(namespace);
	}

	fn drop_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) async fn recv_stream(self, stream: S::RecvStream) -> Result<(), SessionError> {
		let mut reader = Reader::new(stream);
		let header: data::Header = reader.decode().await?;

		// This is super silly, but I couldn't figure out a way to avoid the mutex guard across awaits.
		enum Writer {
			Track(serve::StreamWriter),
			Group(serve::GroupWriter),
			Object(serve::ObjectWriter),
		}

		let writer = {
			let id = header.subscribe_id();
			let mut subscribes = self.subscribes.lock().unwrap();
			let subscribe = match subscribes.get_mut(&id) {
				Some(subscribe) => subscribe,
				None => return Ok(reader.into_inner().stop(1)), // TODO define error codes?
			};

			match header {
				data::Header::Track(track) => Writer::Track(subscribe.recv_track(track)?),
				data::Header::Group(group) => Writer::Group(subscribe.recv_group(group)?),
				data::Header::Object(object) => Writer::Object(subscribe.recv_object(object)?),
			}
		};

		match writer {
			Writer::Track(track) => Self::recv_track(track, reader).await?,
			Writer::Group(group) => Self::recv_group(group, reader).await?,
			Writer::Object(object) => Self::recv_object(object, reader).await?,
		};

		Ok(())
	}

	async fn recv_track(mut track: serve::StreamWriter, mut reader: Reader<S::RecvStream>) -> Result<(), SessionError> {
		log::trace!("received track: {:?}", track);

		let mut prev: Option<serve::StreamGroupWriter> = None;

		while !reader.done().await? {
			let chunk: data::TrackObject = reader.decode().await?;

			let mut group = match prev {
				Some(group) if group.group_id == chunk.group_id => group,
				_ => track.create(chunk.group_id)?,
			};

			let mut object = group.create(chunk.size)?;

			let mut remain = chunk.size;
			while remain > 0 {
				let chunk = reader.read(remain).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received track payload: {:?}", chunk.len());
				remain -= chunk.len();
				object.write(chunk)?;
			}

			prev = Some(group);
		}

		Ok(())
	}

	async fn recv_group(mut group: serve::GroupWriter, mut reader: Reader<S::RecvStream>) -> Result<(), SessionError> {
		log::trace!("received group: {:?}", group.group);

		while !reader.done().await? {
			let object: data::GroupObject = reader.decode().await?;

			log::trace!("received group object: {:?}", object);
			let mut remain = object.size;
			let mut object = group.create(object.size)?;

			while remain > 0 {
				let data = reader.read(remain).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received group payload: {:?}", data.len());
				remain -= data.len();
				object.write(data)?;
			}
		}

		Ok(())
	}

	async fn recv_object(
		mut object: serve::ObjectWriter,
		mut reader: Reader<S::RecvStream>,
	) -> Result<(), SessionError> {
		log::trace!("received object: {:?}", object.object);

		while let Some(data) = reader.read(usize::MAX).await? {
			log::trace!("received object payload: {:?}", data.len());
			object.write(data)?;
		}

		Ok(())
	}

	// TODO should not be async
	pub async fn recv_datagram(&mut self, datagram: bytes::Bytes) -> Result<(), SessionError> {
		let mut cursor = io::Cursor::new(datagram);
		let datagram = data::Datagram::decode(&mut cursor)?;

		if let Some(subscribe) = self.subscribes.lock().unwrap().get_mut(&datagram.subscribe_id) {
			subscribe.recv_datagram(datagram)?;
		}

		Ok(())
	}

	pub fn close(self, err: SessionError) {
		self.outgoing.close(err.clone()).ok();
		self.announced_queue.close(err).ok();
	}
}
