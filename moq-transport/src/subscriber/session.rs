use std::{
	collections::{hash_map, HashMap},
	sync::{atomic, Arc, Mutex},
};

use crate::{
	control, data,
	error::{AnnounceError, SessionError, SubscribeError},
	util::Queue,
};

use super::{Announce, AnnouncePending, AnnounceWeak, Subscribe, SubscribeOptions, SubscribePending, SubscribeWeak};

#[derive(Clone)]
pub struct Session {
	announces: Arc<Mutex<HashMap<String, AnnounceWeak>>>,
	announces_pending: Queue<AnnouncePending, SessionError>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeWeak>>>,
	subscribe_next: Arc<atomic::AtomicU64>,

	messages: Queue<control::Message, SessionError>,
}

impl Session {
	pub fn new() -> Self {
		Self {
			announces: Default::default(),
			announces_pending: Default::default(),
			subscribes: Default::default(),
			subscribe_next: Default::default(),
			messages: Default::default(),
		}
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

	pub(super) fn send_message<M: Into<control::Message>>(&mut self, msg: M) -> Result<(), SessionError> {
		self.messages.push(msg.into())
	}

	pub async fn next_message(&mut self) -> Result<control::Message, SessionError> {
		self.messages.pop().await
	}

	pub fn recv_announce(&mut self, msg: control::Announce) -> Result<(), AnnounceError> {
		let mut announces = self.announces.lock().unwrap();

		let entry = match announces.entry(msg.namespace) {
			hash_map::Entry::Occupied(_) => return Err(AnnounceError::Duplicate),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let announce = Announce::new(self.clone(), msg.namespace);
		entry.insert(announce.clone().downgrade());
		self.announces_pending.push(AnnouncePending::new(announce));

		Ok(())
	}

	pub fn recv_unannounce(&mut self, msg: control::Unannounce) -> Result<(), AnnounceError> {
		let mut announces = self.announces.lock().unwrap();

		if let Some(mut announce) = announces.remove(&msg.namespace) {
			announce.close(AnnounceError::Done)?;
		}

		Ok(())
	}

	pub fn recv_subscribe_ok(&mut self, msg: control::SubscribeOk) -> Result<(), SubscribeError> {
		let mut subscribes = self.subscribes.lock().unwrap();

		if let Some(subscribe) = subscribes.get_mut(&msg.id) {
			subscribe.ok(msg)?;
		}

		Ok(())
	}

	pub fn recv_subscribe_error(&mut self, msg: control::SubscribeError) -> Result<(), SubscribeError> {
		let mut subscribes = self.subscribes.lock().unwrap();

		if let Some(subscribe) = subscribes.get_mut(&msg.id) {
			subscribe.close(SubscribeError::Error(msg.code)).ok();
		}

		Ok(())
	}

	pub fn recv_subscribe_done(&mut self, msg: control::SubscribeDone) -> Result<(), SubscribeError> {
		let mut subscribes = self.subscribes.lock().unwrap();

		if let Some(subscribe) = subscribes.get_mut(&msg.id) {
			subscribe.close(SubscribeError::Done(msg.code)).ok();
		}

		Ok(())
	}

	pub(super) fn remove_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) fn remove_announce(&mut self, namespace: String) {
		self.announces.lock().unwrap().remove(&namespace);
	}

	pub fn recv_data(
		&mut self,
		header: data::Header,
		stream: webtransport_quinn::RecvStream,
	) -> Result<(), SubscribeError> {
		let id = header.subscribe_id();

		let mut subscribes = self.subscribes.lock().unwrap();
		let subscribe = subscribes.get_mut(&id).ok_or(SubscribeError::NotFound)?;

		subscribe.data(header, stream)?;

		Ok(())
	}

	pub fn close(mut self, err: SessionError) {
		self.messages.close(err.clone()).ok();
		self.announces_pending.close(err).ok();
	}
}
