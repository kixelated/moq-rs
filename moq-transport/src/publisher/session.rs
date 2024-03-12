use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex, Weak},
};

use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::{
	coding,
	control::{self, Message},
	session::SessionError,
	setup, MoqError,
};

use super::{Announce, Subscribe, SubscribeRequest, SubscribeState};

#[derive(Clone)]
pub struct Session {
	webtransport: webtransport_quinn::Session,

	announces: Arc<Mutex<HashMap<String, Announce>>>,
	subscribes: Arc<Mutex<HashMap<u64, Weak<Mutex<SubscribeState>>>>>,

	control_send: UnboundedSender<control::Message>,
	control_sink: Arc<Mutex<Option<UnboundedReceiver<control::Message>>>>,
}

impl Session {
	pub fn new(webtransport: webtransport_quinn::Session) -> Self {
		let (control_send, control_sink) = tokio::sync::mpsc::unbounded_channel();

		Self {
			webtransport,
			announces: Default::default(),
			subscribes: Default::default(),
			control_send,
			control_sink: Arc::new(Mutex::new(Some(control_sink))),
		}
	}

	pub async fn announce(&mut self) -> Result<(), SessionError> {
		unimplemented!("announce")
	}

	pub async fn subscribed(&mut self) -> Result<Subscribe, SessionError> {
		unimplemented!("subscribed")
	}

	pub(super) fn send_message<M: Into<control::Message>>(&mut self, msg: M) -> Result<(), SessionError> {
		self.control_send.send(msg.into()).map_err(|_| SessionError::Closed)
	}

	pub fn recv_message(&mut self, msg: control::Message) -> Result<(), SessionError> {
		match msg {
			Message::AnnounceOk(msg) => self.recv_announce_ok(msg),
			Message::AnnounceError(msg) => self.recv_announce_error(msg),
			Message::Subscribe(msg) => self.recv_subscribe(msg),
			Message::Unsubscribe(msg) => self.recv_unsubscribe(msg),
			_ => Err(SessionError::RoleViolation(msg.id())),
		}
	}

	pub(super) fn recv_announce_ok(&mut self, _msg: control::AnnounceOk) -> Result<(), SessionError> {
		unimplemented!("recv_announce_ok")
	}

	pub(super) fn recv_announce_error(&mut self, _msg: control::AnnounceError) -> Result<(), SessionError> {
		unimplemented!("recv_announce_error")
	}

	/*
	fn recv_announce_cancel(&mut self, _msg: control::AnnounceCancel) -> Result<(), SessionError> {
		unimplemented!("recv_announce_cancel")
	}
	*/

	pub(super) fn recv_subscribe(&mut self, msg: control::Subscribe) -> Result<(), SessionError> {
		// Insert the abort handle into the lookup table.
		let entry = match self.subscribes.lock().unwrap().entry(msg.id) {
			hash_map::Entry::Occupied(_) => return Err(SessionError::Duplicate.into()),
			hash_map::Entry::Vacant(entry) => entry,
		};

		let state = SubscribeState::new(self.clone(), msg.id);
		entry.insert(Arc::downgrade(&state));

		let subscribe = Subscribe::new(self.clone(), msg, state);
		let req = SubscribeRequest::new(subscribe);

		Ok(())
	}

	pub(super) fn recv_unsubscribe(&mut self, msg: control::Unsubscribe) -> Result<(), SessionError> {
		if let Some(subscribe) = self.subscribes.lock().unwrap().get_mut(&msg.id) {
			if let Some(subscribe) = subscribe.upgrade() {
				// TODO better error code
				subscribe.lock().unwrap().close(SessionError::Unsubscribe).ok();
			}
		}

		Ok(())
	}

	pub async fn next_message(&mut self) -> Result<control::Message, SessionError> {
		let mut sink = self
			.control_sink
			.lock()
			.unwrap()
			.take()
			.expect("only one reader at a time");

		let res = sink.recv().await;

		self.control_sink.lock().unwrap().replace(sink);

		res.ok_or(SessionError::Closed)
	}

	pub(super) fn remove_subscribe(&mut self, id: u64) {
		self.subscribes.lock().unwrap().remove(&id);
	}

	pub(super) async fn open_uni(&mut self) -> Result<webtransport_quinn::SendStream, SessionError> {
		let stream = self.webtransport.open_uni().await?;
		Ok(stream)
	}
}
