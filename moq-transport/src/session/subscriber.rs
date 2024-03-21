use std::{
	collections::{hash_map, HashMap},
	io,
	sync::{atomic, Arc, Mutex},
};

use crate::{data, message, setup, util::Queue};

use super::{Announced, AnnouncedRecv, Session, SessionError, Subscribe, SubscribeOptions, SubscribeRecv};

// TODO remove Clone.
#[derive(Clone)]
pub struct Subscriber<S: webtransport_generic::Session> {
	announced: Arc<Mutex<HashMap<String, AnnouncedRecv<S>>>>,
	announced_queue: Queue<Announced<S>, SessionError<S>>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeRecv<S>>>>,
	subscribe_next: Arc<atomic::AtomicU64>,

	outgoing: Queue<message::Message, SessionError<S>>,
}

impl<S: webtransport_generic::Session> Subscriber<S> {
	pub(super) fn new(outgoing: Queue<message::Message, SessionError<S>>) -> Self {
		Self {
			announced: Default::default(),
			announced_queue: Default::default(),
			subscribes: Default::default(),
			subscribe_next: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: S) -> Result<(Session<S>, Self), SessionError<S>> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: S) -> Result<(Session<S>, Self), SessionError<S>> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Result<Announced<S>, SessionError<S>> {
		self.announced_queue.pop().await
	}

	pub fn subscribe(
		&mut self,
		namespace: &str,
		name: &str,
		options: SubscribeOptions,
	) -> Result<Subscribe<S>, SessionError<S>> {
		let id = self.subscribe_next.fetch_add(1, atomic::Ordering::Relaxed);

		let msg = message::Subscribe {
			id,
			track_alias: id,
			track_namespace: namespace.to_string(),
			track_name: name.to_string(),
			start: options.start,
			end: options.end,
			params: Default::default(),
		};

		self.send_message(msg.clone())?;

		let (publisher, subscribe) = Subscribe::new(self.clone(), msg);
		self.subscribes.lock().unwrap().insert(id, publisher);

		Ok(subscribe)
	}

	pub(super) fn send_message<M: Into<message::Subscriber>>(&mut self, msg: M) -> Result<(), SessionError<S>> {
		let msg = msg.into();
		log::debug!("sending message: {:?}", msg);
		self.outgoing.push(msg.into())
	}

	pub(super) fn recv_message(&mut self, msg: message::Publisher) -> Result<(), SessionError<S>> {
		log::debug!("received message: {:?}", msg);

		match msg {
			message::Publisher::Announce(msg) => self.recv_announce(msg),
			message::Publisher::Unannounce(msg) => self.recv_unannounce(msg),
			message::Publisher::SubscribeOk(msg) => self.recv_subscribe_ok(msg),
			message::Publisher::SubscribeError(msg) => self.recv_subscribe_error(msg),
			message::Publisher::SubscribeDone(msg) => self.recv_subscribe_done(msg),
		}
	}

	fn recv_announce(&mut self, msg: message::Announce) -> Result<(), SessionError<S>> {
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

	fn recv_unannounce(&mut self, msg: message::Unannounce) -> Result<(), SessionError<S>> {
		if let Some(announce) = self.announced.lock().unwrap().get_mut(&msg.namespace) {
			announce.recv_unannounce().ok();
		}

		Ok(())
	}

	fn recv_subscribe_ok(&mut self, msg: message::SubscribeOk) -> Result<(), SessionError<S>> {
		if let Some(sub) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			sub.recv_ok(msg).ok();
		}

		Ok(())
	}

	fn recv_subscribe_error(&mut self, msg: message::SubscribeError) -> Result<(), SessionError<S>> {
		if let Some(subscriber) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			subscriber.recv_error(msg.code).ok();
		}

		Ok(())
	}

	fn recv_subscribe_done(&mut self, msg: message::SubscribeDone) -> Result<(), SessionError<S>> {
		if let Some(subscriber) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			subscriber.recv_done(msg.code).ok();
		}

		Ok(())
	}

	pub(super) fn drop_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) fn drop_announce(&mut self, namespace: &str) {
		self.announced.lock().unwrap().remove(namespace);
	}

	pub(super) async fn recv_stream(self, mut stream: S::RecvStream) -> Result<(), SessionError<S>> {
		let header = data::Header::decode(&mut stream).await?;

		let id = header.subscribe_id();
		let subscribe = self.subscribes.lock().unwrap().get(&id).cloned();

		if let Some(mut subscribe) = subscribe {
			subscribe.recv_stream(header, stream).await?
		}

		Ok(())
	}

	// TODO should not be async
	pub async fn recv_datagram(&mut self, datagram: bytes::Bytes) -> Result<(), SessionError<S>> {
		let mut cursor = io::Cursor::new(datagram);
		let datagram = data::Datagram::decode(&mut cursor).await?;

		let subscribe = self.subscribes.lock().unwrap().get(&datagram.subscribe_id).cloned();

		if let Some(subscribe) = subscribe {
			subscribe.recv_datagram(datagram)?;
		}

		Ok(())
	}

	pub fn close(self, err: SessionError<S>) {
		self.outgoing.close(err.clone()).ok();
		self.announced_queue.close(err).ok();
	}
}
