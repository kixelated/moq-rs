use std::{
	collections::{hash_map, HashMap},
	sync::{atomic, Arc, Mutex},
};

use crate::{
	control, data,
	error::{AnnounceError, SessionError, SubscribeError},
	setup,
	util::Queue,
	Session,
};

use super::{Announce, AnnouncePending, AnnounceWeak, Subscribe, SubscribeOptions, SubscribePending, SubscribeWeak};

#[derive(Clone)]
pub struct Subscriber {
	announces: Arc<Mutex<HashMap<String, AnnounceWeak>>>,
	announces_pending: Queue<AnnouncePending, SessionError>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeWeak>>>,
	subscribe_next: Arc<atomic::AtomicU64>,

	outgoing: Queue<control::Message, SessionError>,
}

impl Subscriber {
	pub(crate) fn new(outgoing: Queue<control::Message, SessionError>) -> Self {
		Self {
			announces: Default::default(),
			announces_pending: Default::default(),
			subscribes: Default::default(),
			subscribe_next: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: webtransport_quinn::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::accept_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn connect(session: webtransport_quinn::Session) -> Result<(Session, Self), SessionError> {
		let (session, _, subscriber) = Session::connect_role(session, setup::Role::Subscriber).await?;
		Ok((session, subscriber.unwrap()))
	}

	pub async fn announced(&mut self) -> Result<AnnouncePending, SessionError> {
		self.announces_pending.pop().await
	}

	pub async fn subscribe(
		&mut self,
		namespace: String,
		name: String,
		options: SubscribeOptions,
	) -> Result<SubscribePending, SessionError> {
		let id = self.subscribe_next.fetch_add(1, atomic::Ordering::Relaxed);

		let msg = control::Subscribe {
			id,
			track_alias: id,
			track_namespace: namespace,
			track_name: name,
			start: options.start,
			end: options.end,
			params: Default::default(),
		};

		self.send_message(msg.clone())?;

		let subscribe = Subscribe::new(self.clone(), msg);
		self.subscribes
			.lock()
			.unwrap()
			.insert(id, subscribe.clone().downgrade());
		let pending = SubscribePending::new(subscribe.clone());

		Ok(pending)
	}

	pub(super) fn send_message<M: Into<control::Subscriber> + Into<control::Message>>(
		&mut self,
		msg: M,
	) -> Result<(), SessionError> {
		self.outgoing.push(msg.into())
	}

	pub(crate) fn recv_message(&mut self, msg: control::Publisher) -> Result<(), SessionError> {
		match msg {
			control::Publisher::Announce(msg) => self.recv_announce(msg),
			control::Publisher::Unannounce(msg) => self.recv_unannounce(msg),
			control::Publisher::SubscribeOk(msg) => self.recv_subscribe_ok(msg),
			control::Publisher::SubscribeError(msg) => self.recv_subscribe_error(msg),
			control::Publisher::SubscribeDone(msg) => self.recv_subscribe_done(msg),
		}
	}

	fn recv_announce(&mut self, msg: control::Announce) -> Result<(), SessionError> {
		let mut announces = self.announces.lock().unwrap();

		let entry = match announces.entry(msg.namespace.clone()) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let announce = Announce::new(self.clone(), msg.namespace);
		entry.insert(announce.downgrade());

		self.announces_pending.push(AnnouncePending::new(announce))?;

		Ok(())
	}

	fn recv_unannounce(&mut self, msg: control::Unannounce) -> Result<(), SessionError> {
		if let Some(mut announce) = self.get_announce(&msg.namespace) {
			announce.close(AnnounceError::Done).ok();
		}

		Ok(())
	}

	fn recv_subscribe_ok(&mut self, msg: control::SubscribeOk) -> Result<(), SessionError> {
		if let Some(mut sub) = self.get_subscribe(msg.id) {
			sub.recv_ok(msg).ok();
		}

		Ok(())
	}

	fn recv_subscribe_error(&mut self, msg: control::SubscribeError) -> Result<(), SessionError> {
		if let Some(mut subscriber) = self.get_subscribe(msg.id) {
			subscriber.recv_error(SubscribeError::Error(msg.code)).ok();
		}

		Ok(())
	}

	fn recv_subscribe_done(&mut self, msg: control::SubscribeDone) -> Result<(), SessionError> {
		if let Some(mut subscriber) = self.get_subscribe(msg.id) {
			subscriber.recv_error(SubscribeError::Done(msg.code)).ok();
		}

		Ok(())
	}

	fn get_announce(&self, namespace: &str) -> Option<Announce> {
		self.announces.lock().unwrap().get(namespace)?.upgrade()
	}

	fn get_subscribe(&self, id: u64) -> Option<Subscribe> {
		self.subscribes.lock().unwrap().get(&id)?.upgrade()
	}

	pub(super) fn drop_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) fn drop_announce(&mut self, namespace: &str) {
		self.announces.lock().unwrap().remove(namespace);
	}

	pub fn recv_stream(
		&mut self,
		header: data::Header,
		stream: webtransport_quinn::RecvStream,
	) -> Result<(), SessionError> {
		let id = header.subscribe_id();
		if let Some(mut subscribe) = self.get_subscribe(id) {
			// TODO handle some of these errors?
			subscribe.recv_stream(header, stream).ok();
		}

		Ok(())
	}

	pub fn close(self, err: SessionError) {
		self.outgoing.close(err.clone()).ok();
		self.announces_pending.close(err).ok();
	}
}
