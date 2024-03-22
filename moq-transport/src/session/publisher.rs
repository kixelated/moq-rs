use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};

use crate::{
	message::{self, Message},
	serve::{self, ServeError},
	setup,
	util::Queue,
};

use super::{Announce, AnnounceRecv, Session, SessionError, Subscribed, SubscribedRecv};

// TODO remove Clone.
#[derive(Clone)]
pub struct Publisher<S: webtransport_generic::Session> {
	webtransport: S,

	announces: Arc<Mutex<HashMap<String, AnnounceRecv>>>,
	subscribed: Arc<Mutex<HashMap<u64, SubscribedRecv<S>>>>,
	subscribed_queue: Queue<Subscribed<S>, SessionError>,

	outgoing: Queue<Message, SessionError>,
}

impl<S: webtransport_generic::Session> Publisher<S> {
	pub(crate) fn new(webtransport: S, outgoing: Queue<Message, SessionError>) -> Self {
		Self {
			webtransport,
			announces: Default::default(),
			subscribed: Default::default(),
			subscribed_queue: Default::default(),
			outgoing,
		}
	}

	pub async fn accept(session: S) -> Result<(Session<S>, Publisher<S>), SessionError> {
		let (session, publisher, _) = Session::accept_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub async fn connect(session: S) -> Result<(Session<S>, Publisher<S>), SessionError> {
		let (session, publisher, _) = Session::connect_role(session, setup::Role::Publisher).await?;
		Ok((session, publisher.unwrap()))
	}

	pub fn announce(&mut self, namespace: &str) -> Result<Announce<S>, SessionError> {
		let mut announces = self.announces.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match announces.entry(namespace.to_string()) {
			hash_map::Entry::Occupied(_) => return Err(ServeError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let msg = message::Announce {
			namespace: namespace.to_string(),
			params: Default::default(),
		};
		self.send_message(msg.clone())?;

		let (announce, recv) = Announce::new(self.clone(), msg);
		entry.insert(recv);

		Ok(announce)
	}

	pub async fn subscribed(&mut self) -> Result<Subscribed<S>, SessionError> {
		self.subscribed_queue.pop().await
	}

	// Helper to announce and serve any matching subscribers.
	// TODO this currently takes over the connection; definitely remove Clone
	pub async fn serve(mut self, broadcast: serve::BroadcastSubscriber) -> Result<(), SessionError> {
		log::info!("serving broadcast: {}", broadcast.namespace);

		let announce = self.announce(&broadcast.namespace)?;
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				err = announce.closed() => err?,
				res = tasks.next(), if !tasks.is_empty() => {
					// TODO preseve the track name too
					log::debug!("served track: namespace={} res={:?}", broadcast.namespace, res);
				},
				sub = self.subscribed() => {
					let mut subscribe = sub?;
					match self.serve_track(&broadcast, &subscribe) {
						Ok(track) => {
							log::info!("serving track: namespace={} name={}", track.namespace, track.name);
							tasks.push(subscribe.serve(track).boxed());
						},
						Err(err) => {
							log::debug!("failed serving track: namespace={} name={} err={}", subscribe.namespace(), subscribe.name(), err);
							subscribe.close(err).ok();
						}
					};
				}
			}
		}
	}

	fn serve_track(
		&self,
		broadcast: &serve::BroadcastSubscriber,
		subscribe: &Subscribed<S>,
	) -> Result<serve::TrackSubscriber, ServeError> {
		if subscribe.namespace() != broadcast.namespace {
			return Err(ServeError::NotFound);
		}

		broadcast.get_track(subscribe.name())?.ok_or(ServeError::NotFound)
	}

	pub(crate) fn recv_message(&mut self, msg: message::Subscriber) -> Result<(), SessionError> {
		log::debug!("received message: {:?}", msg);

		match msg {
			message::Subscriber::AnnounceOk(msg) => self.recv_announce_ok(msg),
			message::Subscriber::AnnounceError(msg) => self.recv_announce_error(msg),
			message::Subscriber::AnnounceCancel(msg) => self.recv_announce_cancel(msg),
			message::Subscriber::Subscribe(msg) => self.recv_subscribe(msg),
			message::Subscriber::Unsubscribe(msg) => self.recv_unsubscribe(msg),
		}
	}

	fn recv_announce_ok(&mut self, _msg: message::AnnounceOk) -> Result<(), SessionError> {
		// Who cares
		// TODO make AnnouncePending so we're forced to care
		Ok(())
	}

	fn recv_announce_error(&mut self, msg: message::AnnounceError) -> Result<(), SessionError> {
		if let Some(announce) = self.announces.lock().unwrap().get_mut(&msg.namespace) {
			announce.recv_error(ServeError::Closed(msg.code)).ok();
		}

		Ok(())
	}

	fn recv_announce_cancel(&mut self, _msg: message::AnnounceCancel) -> Result<(), SessionError> {
		unimplemented!("recv_announce_cancel")
	}

	fn recv_subscribe(&mut self, msg: message::Subscribe) -> Result<(), SessionError> {
		let mut subscribes = self.subscribed.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match subscribes.entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let (subscribe, recv) = Subscribed::new(self.clone(), msg);
		entry.insert(recv);
		self.subscribed_queue.push(subscribe)
	}

	fn recv_unsubscribe(&mut self, msg: message::Unsubscribe) -> Result<(), SessionError> {
		if let Some(subscribed) = self.subscribed.lock().unwrap().get_mut(&msg.id) {
			subscribed.recv_unsubscribe().ok();
		}

		Ok(())
	}

	pub fn send_message<T: Into<message::Publisher>>(&self, msg: T) -> Result<(), SessionError> {
		let msg = msg.into();
		log::debug!("sending message: {:?}", msg);
		self.outgoing.push(msg.into())
	}

	pub(super) fn drop_subscribe(&mut self, id: u64) {
		self.subscribed.lock().unwrap().remove(&id);
	}

	pub(super) fn drop_announce(&mut self, namespace: &str) {
		self.announces.lock().unwrap().remove(namespace);
	}

	pub(super) fn webtransport(&mut self) -> &mut S {
		&mut self.webtransport
	}

	pub fn close(self, err: SessionError) {
		self.outgoing.close(err.clone()).ok();
		self.subscribed_queue.close(err).ok();
	}
}
