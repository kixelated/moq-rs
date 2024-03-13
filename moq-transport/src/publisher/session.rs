use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use crate::{
	control,
	error::{AnnounceError, SessionError, SubscribeError},
	util::Queue,
};

use super::{Announce, AnnounceWeak, Subscribe, SubscribePending, SubscribeWeak};

#[derive(Clone)]
pub struct Session {
	announces: Arc<Mutex<HashMap<String, AnnounceWeak>>>,

	subscribes: Arc<Mutex<HashMap<u64, SubscribeWeak>>>,
	subscribes_pending: Queue<SubscribePending, SessionError>,

	messages: Queue<control::Message, SessionError>,
}

impl Session {
	pub fn new() -> Self {
		Self {
			announces: Default::default(),
			subscribes: Default::default(),
			messages: Default::default(),
			subscribes_pending: Default::default(),
		}
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

	pub(super) fn send_message<M: Into<control::Message>>(&mut self, msg: M) -> Result<(), SessionError> {
		self.messages.push(msg.into())
	}

	pub fn recv_announce_ok(&mut self, _msg: control::AnnounceOk) -> Result<(), AnnounceError> {
		// Who cares
		// TODO make AnnouncePending so we're forced to care
		Ok(())
	}

	pub fn recv_announce_error(&mut self, msg: control::AnnounceError) -> Result<(), AnnounceError> {
		if let Some(announce) = self.announces.lock().unwrap().get_mut(&msg.namespace) {
			announce.close(SessionError::Reset(msg.code))?;
		}

		Ok(())
	}

	pub fn recv_announce_cancel(&mut self, _msg: control::AnnounceCancel) -> Result<(), AnnounceError> {
		unimplemented!("recv_announce_cancel")
	}

	pub fn recv_subscribe(&mut self, msg: control::Subscribe) -> Result<(), SubscribeError> {
		let mut subscribes = self.subscribes.lock().unwrap();

		// Insert the abort handle into the lookup table.
		let entry = match subscribes.entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(SubscribeError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let subscribe = Subscribe::new(self.clone(), msg);
		entry.insert(subscribe.downgrade());

		let pending = SubscribePending::new(subscribe);
		self.subscribes_pending.push(pending)
	}

	pub fn recv_unsubscribe(&mut self, msg: control::Unsubscribe) -> Result<(), SubscribeError> {
		if let Some(subscribe) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			subscribe.close(SessionError::Stop)?;
		}

		Ok(())
	}

	pub async fn next_message(&mut self) -> Result<control::Message, SessionError> {
		self.messages.pop().await
	}

	pub(super) fn remove_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) fn remove_announce(&mut self, namespace: String) {
		self.announces.lock().unwrap().remove(&namespace);
	}

	pub fn close(mut self, err: SessionError) {
		self.messages.close(err.clone()).ok();
		self.subscribes_pending.close(err).ok();
	}
}
