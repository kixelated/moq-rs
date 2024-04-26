use std::{
	collections::{hash_map, HashMap},
	io,
	sync::{atomic, Arc, Mutex},
};

use crate::{
	coding::Decode,
	data,
	message::{self, Message},
	serve::{self, ServeError},
	setup,
};

use crate::watch::Queue;

use super::{Announced, AnnouncedRecv, Reader, Session, SessionError, Subscribe, SubscribeRecv};

// TODO remove Clone.
#[derive(Clone)]
pub struct Subscriber {
	announced: Arc<Mutex<HashMap<String, AnnouncedRecv>>>,
	announced_queue: Queue<Announced>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeRecv>>>,
	subscribe_next: Arc<atomic::AtomicU64>,

	outgoing: Queue<Message>,
}

impl Subscriber {
	pub(super) fn new(outgoing: Queue<Message>) -> Self {
		Self {
			announced: Default::default(),
			announced_queue: Default::default(),
			subscribes: Default::default(),
			subscribe_next: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: web_transport::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Option<Announced> {
		self.announced_queue.pop().await
	}

	pub async fn subscribe(&mut self, track: serve::TrackWriter) -> Result<(), ServeError> {
		let id = self.subscribe_next.fetch_add(1, atomic::Ordering::Relaxed);

		let (send, recv) = Subscribe::new(self.clone(), id, track);
		self.subscribes.lock().unwrap().insert(id, recv);

		send.closed().await
	}

	pub(super) fn send_message<M: Into<message::Subscriber>>(&mut self, msg: M) {
		let msg = msg.into();

		// Remove our entry on terminal state.
		match &msg {
			message::Subscriber::AnnounceCancel(msg) => self.drop_announce(&msg.namespace),
			message::Subscriber::AnnounceError(msg) => self.drop_announce(&msg.namespace),
			_ => {}
		}

		// TODO report dropped messages?
		let _ = self.outgoing.push(msg.into());
	}

	pub(super) fn recv_message(&mut self, msg: message::Publisher) -> Result<(), SessionError> {
		let res = match &msg {
			message::Publisher::Announce(msg) => self.recv_announce(msg),
			message::Publisher::Unannounce(msg) => self.recv_unannounce(msg),
			message::Publisher::SubscribeOk(msg) => self.recv_subscribe_ok(msg),
			message::Publisher::SubscribeError(msg) => self.recv_subscribe_error(msg),
			message::Publisher::SubscribeDone(msg) => self.recv_subscribe_done(msg),
		};

		if let Err(SessionError::Serve(err)) = res {
			log::debug!("failed to process message: {:?} {}", msg, err);
			return Ok(());
		}

		res
	}

	fn recv_announce(&mut self, msg: &message::Announce) -> Result<(), SessionError> {
		let mut announces = self.announced.lock().unwrap();

		let entry = match announces.entry(msg.namespace.clone()) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let (announced, recv) = Announced::new(self.clone(), msg.namespace.to_string());
		if let Err(announced) = self.announced_queue.push(announced) {
			announced.close(ServeError::Cancel)?;
			return Ok(());
		}

		entry.insert(recv);

		Ok(())
	}

	fn recv_unannounce(&mut self, msg: &message::Unannounce) -> Result<(), SessionError> {
		if let Some(announce) = self.announced.lock().unwrap().remove(&msg.namespace) {
			announce.recv_unannounce()?;
		}

		Ok(())
	}

	fn recv_subscribe_ok(&mut self, msg: &message::SubscribeOk) -> Result<(), SessionError> {
		if let Some(subscribe) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			subscribe.ok()?;
		}

		Ok(())
	}

	fn recv_subscribe_error(&mut self, msg: &message::SubscribeError) -> Result<(), SessionError> {
		if let Some(subscribe) = self.subscribes.lock().unwrap().remove(&msg.id) {
			subscribe.error(ServeError::Closed(msg.code))?;
		}

		Ok(())
	}

	fn recv_subscribe_done(&mut self, msg: &message::SubscribeDone) -> Result<(), SessionError> {
		if let Some(subscribe) = self.subscribes.lock().unwrap().remove(&msg.id) {
			subscribe.error(ServeError::Closed(msg.code))?;
		}

		Ok(())
	}

	fn drop_announce(&mut self, namespace: &str) {
		self.announced.lock().unwrap().remove(namespace);
	}

	pub(super) async fn recv_stream(mut self, stream: web_transport::RecvStream) -> Result<(), SessionError> {
		let mut reader = Reader::new(stream);
		let header: data::Header = reader.decode().await?;

		let id = header.subscribe_id();

		let res = self.recv_stream_inner(reader, header).await;
		if let Err(SessionError::Serve(err)) = &res {
			// The writer is closed, so we should teriminate.
			// TODO it would be nice to do this immediately when the Writer is closed.
			if let Some(subscribe) = self.subscribes.lock().unwrap().remove(&id) {
				subscribe.error(err.clone())?;
			}
		}

		res
	}

	async fn recv_stream_inner(&mut self, reader: Reader, header: data::Header) -> Result<(), SessionError> {
		let id = header.subscribe_id();

		// This is super silly, but I couldn't figure out a way to avoid the mutex guard across awaits.
		enum Writer {
			Track(serve::StreamWriter),
			Group(serve::GroupWriter),
			Object(serve::ObjectWriter),
		}

		let writer = {
			let mut subscribes = self.subscribes.lock().unwrap();
			let subscribe = subscribes.get_mut(&id).ok_or(ServeError::NotFound)?;

			match header {
				data::Header::Track(track) => Writer::Track(subscribe.track(track)?),
				data::Header::Group(group) => Writer::Group(subscribe.group(group)?),
				data::Header::Object(object) => Writer::Object(subscribe.object(object)?),
			}
		};

		match writer {
			Writer::Track(track) => Self::recv_track(track, reader).await?,
			Writer::Group(group) => Self::recv_group(group, reader).await?,
			Writer::Object(object) => Self::recv_object(object, reader).await?,
		};

		Ok(())
	}

	async fn recv_track(mut track: serve::StreamWriter, mut reader: Reader) -> Result<(), SessionError> {
		log::trace!("received track: {:?}", track.info);

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
				let chunk = reader.read_chunk(remain).await?.ok_or(SessionError::WrongSize)?;

				log::trace!("received track payload: {:?}", chunk.len());
				remain -= chunk.len();
				object.write(chunk)?;
			}

			prev = Some(group);
		}

		Ok(())
	}

	async fn recv_group(mut group: serve::GroupWriter, mut reader: Reader) -> Result<(), SessionError> {
		log::trace!("received group: {:?}", group.info);

		while !reader.done().await? {
			let object: data::GroupObject = reader.decode().await?;

			log::trace!("received group object: {:?}", object);
			let mut remain = object.size;
			let mut object = group.create(object.size)?;

			while remain > 0 {
				let data = reader.read_chunk(remain).await?.ok_or(SessionError::WrongSize)?;
				log::trace!("received group payload: {:?}", data.len());
				remain -= data.len();
				object.write(data)?;
			}
		}

		Ok(())
	}

	async fn recv_object(mut object: serve::ObjectWriter, mut reader: Reader) -> Result<(), SessionError> {
		log::trace!("received object: {:?}", object.info);

		while let Some(data) = reader.read_chunk(usize::MAX).await? {
			log::trace!("received object payload: {:?}", data.len());
			object.write(data)?;
		}

		Ok(())
	}

	pub fn recv_datagram(&mut self, datagram: bytes::Bytes) -> Result<(), SessionError> {
		let mut cursor = io::Cursor::new(datagram);
		let datagram = data::Datagram::decode(&mut cursor)?;

		if let Some(subscribe) = self.subscribes.lock().unwrap().get_mut(&datagram.subscribe_id) {
			subscribe.datagram(datagram)?;
		}

		Ok(())
	}
}
