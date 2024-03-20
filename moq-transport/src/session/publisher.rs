use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};

use crate::{
	control,
	serve::{self, ServeError},
	setup,
	util::Queue,
};

use super::{Announce, AnnounceRecv, Session, SessionError, Subscribed, SubscribedRecv};

// TODO remove Clone.
#[derive(Clone)]
pub struct Publisher {
	webtransport: webtransport_quinn::Session,

	announces: Arc<Mutex<HashMap<String, AnnounceRecv>>>,
	subscribed: Arc<Mutex<HashMap<u64, SubscribedRecv>>>,
	subscribed_queue: Queue<Subscribed, SessionError>,

	outgoing: Queue<control::Message, SessionError>,
}

impl Publisher {
	pub(crate) fn new(
		webtransport: webtransport_quinn::Session,
		outgoing: Queue<control::Message, SessionError>,
	) -> Self {
		Self {
			webtransport,
			announces: Default::default(),
			subscribed: Default::default(),
			subscribed_queue: Default::default(),
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

	pub fn announce(&mut self, namespace: &str) -> Result<Announce, ServeError> {
		let mut announces = self.announces.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match announces.entry(namespace.to_string()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let (announce, recv) = Announce::new(self.clone(), namespace.to_string());
		entry.insert(recv);

		Ok(announce)
	}

	pub async fn subscribed(&mut self) -> Result<Subscribed, SessionError> {
		self.subscribed_queue.pop().await
	}

	// Helper to announce and serve any matching subscribers.
	// TODO this currently takes over the connection; definitely remove Clone
	pub async fn serve(mut self, broadcast: serve::BroadcastSubscriber) -> Result<(), SessionError> {
		log::debug!("serving broadcast: {}", broadcast.namespace);

		let announce = self.announce(&broadcast.namespace)?;
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				err = announce.closed() => err?,
				res = tasks.next(), if !tasks.is_empty() => log::debug!("served subscription: broadcast={} res={:?}", broadcast.namespace, res),
				res = self.subscribed() => {
					let mut subscribe = res?;

					// TODO support serving multiple namespaces
					if subscribe.namespace() != broadcast.namespace {
						subscribe.close(ServeError::NotFound).ok();
					} else if let Some(track) = broadcast.get_track(subscribe.name())? {
						tasks.push(subscribe.serve(track).boxed());
					} else {
						subscribe.close(ServeError::NotFound).ok();
					}
				}
			}
		}
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
		if let Some(announce) = self.announces.lock().unwrap().get_mut(&msg.namespace) {
			announce.recv_error(ServeError::Closed(msg.code)).ok();
		}

		Ok(())
	}

	fn recv_announce_cancel(&mut self, _msg: control::AnnounceCancel) -> Result<(), SessionError> {
		unimplemented!("recv_announce_cancel")
	}

	fn recv_subscribe(&mut self, msg: control::Subscribe) -> Result<(), SessionError> {
		let mut subscribes = self.subscribed.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match subscribes.entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let (subscribe, recv) = Subscribed::new(self.clone(), msg);
		entry.insert(recv);
		self.subscribed_queue.push(subscribe)
	}

	fn recv_unsubscribe(&mut self, msg: control::Unsubscribe) -> Result<(), SessionError> {
		if let Some(subscribed) = self.subscribed.lock().unwrap().get_mut(&msg.id) {
			subscribed.recv_unsubscribe().ok();
		}

		Ok(())
	}

	pub fn send_message<T: Into<control::Publisher> + Into<control::Message>>(
		&self,
		msg: T,
	) -> Result<(), SessionError> {
		self.outgoing.push(msg.into())
	}

	pub(super) fn drop_subscribe(&mut self, id: u64) {
		self.subscribed.lock().unwrap().remove(&id);
	}

	pub(super) fn drop_announce(&mut self, namespace: &str) {
		self.announces.lock().unwrap().remove(namespace);
	}

	pub(super) fn webtransport(&mut self) -> &mut webtransport_quinn::Session {
		&mut self.webtransport
	}

	pub fn close(self, err: SessionError) {
		self.outgoing.close(err.clone()).ok();
		self.subscribed_queue.close(err).ok();
	}
}
