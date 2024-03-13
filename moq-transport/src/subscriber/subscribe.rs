use std::{
	collections::VecDeque,
	sync::{Arc, Mutex},
};

use crate::{
	control::{self, SubscribePair},
	data,
	error::SubscribeError,
	util::{Watch, WatchWeak},
};

use super::{GroupStream, ObjectStream, Session, Stream, TrackStream};

#[derive(Clone)]
pub struct Subscribe {
	session: Session,
	msg: control::Subscribe,
	state: Watch<SubscribeState>,
}

impl Subscribe {
	pub(super) fn new(session: Session, msg: control::Subscribe) -> Self {
		let state = SubscribeState::new(session.clone(), msg.id);
		let state = Watch::new(state);

		Self { session, msg, state }
	}

	pub fn namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub async fn stream(&mut self) -> Result<Stream, SubscribeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.streams.len() > 0 {
					return Ok(state.into_mut().streams.pop_front().unwrap());
				}

				state.closed.clone()?;
				state.changed()
			};

			notify.await
		}
	}

	pub fn close(self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.state.lock_mut().close(err)
	}

	pub fn closed(&self) -> Result<(), SubscribeError> {
		self.state.lock().closed()
	}

	pub(super) fn seen(&mut self, group: u64, object: u64) -> Result<(), SubscribeError> {
		self.state.lock_mut().seen(group, object)
	}

	// Returns the maximum known group and object IDs
	pub fn max(&self) -> Option<(u64, u64)> {
		self.state.lock().max
	}

	pub(super) fn downgrade(&self) -> SubscribeWeak {
		SubscribeWeak {
			state: Arc::downgrade(&self.state),
		}
	}
}

pub(super) struct SubscribeWeak {
	state: WatchWeak<SubscribeState>,
}

impl SubscribeWeak {
	pub fn accept(&mut self, stream: webtransport_quinn::RecvStream) -> Result<(), SubscribeError> {
		if let Some(state) = self.state.upgrade() {
			state.lock_mut().accept(stream)
		} else {
			Err(SubscribeError::Dropped)
		}
	}

	pub fn ok(&mut self, msg: control::SubscribeOk) -> Result<(), SubscribeError> {
		if let Some(state) = self.state.upgrade() {
			state.lock_mut().ok(msg)
		} else {
			Err(SubscribeError::Dropped)
		}
	}

	pub fn close(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		if let Some(state) = self.state.upgrade() {
			state.lock_mut().close(err)
		} else {
			Err(SubscribeError::Dropped)
		}
	}
}

struct SubscribeState {
	session: Session,
	msg: control::Subscribe,

	ok: Option<control::SubscribeOk>,
	max: Option<(u64, u64)>,
	closed: Result<(), SubscribeError>,
	streams: VecDeque<Stream>,
}

impl SubscribeState {
	fn new(session: Session, msg: control::Subscribe) -> Arc<Mutex<Self>> {
		Arc::new(Mutex::new(Self {
			session,
			msg,
			ok: None,
			max: None,
			closed: Ok(()),
			streams: VecDeque::new(),
		}))
	}

	pub fn close(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.closed()?;
		self.closed = Err(err.clone());
		self.session.remove_subscribe(self.id);

		Ok(())
	}

	pub fn closed(&self) -> Result<(), SubscribeError> {
		self.closed.clone()
	}

	pub fn ok(&mut self, msg: control::SubscribeOk) -> Result<(), SubscribeError> {
		self.closed()?;
		self.ok = Some(msg);
		self.max = msg.latest;
		Ok(())
	}

	pub fn recv_stream(
		&mut self,
		header: data::Header,
		stream: webtransport_quinn::RecvStream,
	) -> Result<(), SubscribeError> {
		self.closed()?;

		let stream = match header {
			data::Header::Track(header) => TrackStream::new(self.clone(), header, stream).into(),
			data::Header::Group(header) => GroupStream::new(self.clone(), header, stream).into(),
			data::Header::Object(header) => ObjectStream::new(self.clone(), header, stream).into(),
			data::Header::Datagram(_) => panic!("datagram only"),
		};

		self.streams.push_back(stream);

		Ok(())
	}

	pub fn seen(&mut self, group: u64, object: u64) -> Result<(), SubscribeError> {
		self.closed()?;

		if let Some((prev_group, prev_object)) = self.max {
			if prev_group > group || (prev_group == group && prev_object >= object) {
				return Ok(());
			}
		}

		self.max = Some((group, object));

		Ok(())
	}
}

impl Drop for SubscribeState {
	fn drop(&mut self) {
		if self.close(SubscribeError::Stop).is_ok() {
			let msg = control::Unsubscribe { id: self.msg.id };
			self.session.send_message(msg).ok()
		}
	}
}

pub struct SubscribePending {
	subscribe: Subscribe,
}

impl SubscribePending {
	pub(crate) fn new(subscribe: Subscribe) -> Self {
		Self { subscribe }
	}

	pub fn namespace(&self) -> &str {
		self.subscribe.namespace()
	}

	pub fn name(&self) -> &str {
		self.subscribe.name()
	}

	// Wait for a SUBSCRIBE_OK or SUBSCRIBE_ERROR
	pub async fn ready(self) -> Result<Subscribe, SubscribeError> {
		loop {
			let notify = {
				let state = self.subscribe.state.lock();
				state.closed.clone()?;

				if state.ok.is_some() {
					drop(state);
					return Ok(self.subscribe);
				}

				state.changed()
			};

			notify.await
		}
	}
}

#[derive(Default)]
pub struct SubscribeOptions {
	pub start: SubscribePair,
	pub end: SubscribePair,
}
