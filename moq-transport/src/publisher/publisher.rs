use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use crate::{
	control,
	error::{AnnounceError, SessionError, SubscribeError},
	setup,
	util::Queue,
	Session,
};

use super::{Announce, AnnounceWeak, Subscribe, SubscribePending, SubscribeWeak};

#[derive(Clone)]
pub struct Publisher {
	announces: Arc<Mutex<HashMap<String, AnnounceWeak>>>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeWeak>>>,
	subscribes_pending: Queue<SubscribePending, SessionError>,

	outgoing: Queue<control::Message, SessionError>,
}

impl Publisher {
	pub(crate) fn new(outgoing: Queue<control::Message, SessionError>) -> Self {
		Self {
			announces: Default::default(),
			subscribes: Default::default(),
			subscribes_pending: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: webtransport_quinn::Session) -> Result<(Session, Self), SessionError> {
		let (session, publisher, _) = Session::accept_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub async fn connect(session: webtransport_quinn::Session) -> Result<(Session, Self), SessionError> {
		let (session, publisher, _) = Session::connect_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub async fn announce(&mut self, namespace: String) -> Result<Announce, AnnounceError> {
		let mut announces = self.announces.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match announces.entry(namespace.clone()) {
			hash_map::Entry::Occupied(_) => return Err(AnnounceError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let announce = Announce::new(self.clone(), namespace);
		entry.insert(announce.clone().downgrade());

		Ok(announce)
	}

	pub async fn subscribed(&mut self) -> Result<SubscribePending, SessionError> {
		self.subscribes_pending.pop().await
	}

	pub(crate) fn recv_message(&mut self, msg: control::Subscriber) -> Result<(), SessionError> {
		match msg {
			control::Subscriber::AnnounceOk(msg) => self.recv_announce_ok(msg),
			control::Subscriber::AnnounceError(msg) => self.recv_announce_error(msg),
			control::Subscriber::AnnounceCancel(msg) => self.recv_announce_cancel(msg),
			control::Subscriber::Subscribe(msg) => self.recv_subscribe(msg),
			control::Subscriber::Unsubscribe(msg) => self.recv_unsubscribe(msg),
		}
	}

	fn recv_announce_ok(&mut self, _msg: control::AnnounceOk) -> Result<(), SessionError> {
		// Who cares
		// TODO make AnnouncePending so we're forced to care
		Ok(())
	}

	fn recv_announce_error(&mut self, msg: control::AnnounceError) -> Result<(), SessionError> {
		if let Some(mut announce) = self.get_announce(&msg.namespace) {
			announce.close(AnnounceError::Error(msg.code)).ok();
		}

		Ok(())
	}

	fn recv_announce_cancel(&mut self, _msg: control::AnnounceCancel) -> Result<(), SessionError> {
		unimplemented!("recv_announce_cancel")
	}

	fn recv_subscribe(&mut self, msg: control::Subscribe) -> Result<(), SessionError> {
		let mut subscribes = self.subscribes.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match subscribes.entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let subscribe = Subscribe::new(self.clone(), msg);
		entry.insert(subscribe.downgrade());

		let pending = SubscribePending::new(subscribe);
		self.subscribes_pending.push(pending)
	}

	fn recv_unsubscribe(&mut self, msg: control::Unsubscribe) -> Result<(), SessionError> {
		if let Some(mut subscribe) = self.get_subscribe(msg.id) {
			subscribe.close(SubscribeError::Cancel).ok();
		}

		Ok(())
	}

	pub fn send_message<T: Into<control::Publisher> + Into<control::Message>>(
		&self,
		msg: T,
	) -> Result<(), SessionError> {
		self.outgoing.push(msg.into())
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

	pub fn close(self, err: SessionError) {
		self.outgoing.close(err.clone()).ok();
		self.subscribes_pending.close(err).ok();
	}
}
