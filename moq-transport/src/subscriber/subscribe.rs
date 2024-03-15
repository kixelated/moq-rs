use std::collections::VecDeque;

use crate::{
	control::{self, SubscribePair},
	data,
	error::SubscribeError,
	util::{Watch, WatchWeak},
};

use super::{Session, Stream};

#[derive(Clone)]
pub struct Subscribe {
	session: Session,
	msg: control::Subscribe,
	state: Watch<SubscribeState>,
}

impl Subscribe {
	pub(super) fn new(session: Session, msg: control::Subscribe) -> Self {
		let state = SubscribeState::new(session.clone(), msg.clone());
		let state = Watch::new(state);

		Self { session, msg, state }
	}

	pub fn track_namespace(&self) -> &str {
		self.msg.track_namespace.as_str()
	}

	pub fn track_name(&self) -> &str {
		self.msg.track_name.as_str()
	}

	pub async fn next_stream(&mut self) -> Result<Stream, SubscribeError> {
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

	pub(super) fn recv_ok(&mut self, msg: control::SubscribeOk) -> Result<(), SubscribeError> {
		self.state.lock_mut().recv_ok(msg)
	}

	pub(super) fn recv_error(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.state.lock_mut().close(err)
	}

	pub(super) fn recv_stream(
		&mut self,
		header: data::Header,
		stream: webtransport_quinn::RecvStream,
	) -> Result<(), SubscribeError> {
		let stream = Stream::new(self.clone(), header, stream);
		self.state.lock_mut().recv_stream(stream)
	}

	pub(super) fn downgrade(&self) -> SubscribeWeak {
		SubscribeWeak {
			state: self.state.downgrade(),
			session: self.session.clone(),
			msg: self.msg.clone(),
		}
	}
}

pub(super) struct SubscribeWeak {
	state: WatchWeak<SubscribeState>,
	session: Session,
	msg: control::Subscribe,
}

impl SubscribeWeak {
	pub fn upgrade(&self) -> Option<Subscribe> {
		Some(Subscribe {
			state: self.state.upgrade()?,
			session: self.session.clone(),
			msg: self.msg.clone(),
		})
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
	fn new(session: Session, msg: control::Subscribe) -> Self {
		Self {
			session,
			msg,
			ok: None,
			max: None,
			closed: Ok(()),
			streams: VecDeque::new(),
		}
	}

	pub fn close(&mut self, err: SubscribeError) -> Result<(), SubscribeError> {
		self.closed()?;
		self.closed = Err(err.clone());

		Ok(())
	}

	pub fn closed(&self) -> Result<(), SubscribeError> {
		self.closed.clone()
	}

	pub fn recv_ok(&mut self, msg: control::SubscribeOk) -> Result<(), SubscribeError> {
		self.closed()?;
		self.max = msg.latest;
		self.ok = Some(msg);
		Ok(())
	}

	pub fn recv_stream(&mut self, stream: Stream) -> Result<(), SubscribeError> {
		self.closed()?;
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
		if self.close(SubscribeError::Cancel).is_ok() {
			let msg = control::Unsubscribe { id: self.msg.id };
			self.session.send_message(msg).ok();
		}

		self.session.drop_subscribe(self.msg.id);
	}
}

pub struct SubscribePending {
	subscribe: Subscribe,
}

impl SubscribePending {
	pub(crate) fn new(subscribe: Subscribe) -> Self {
		Self { subscribe }
	}

	pub fn track_namespace(&self) -> &str {
		self.subscribe.track_namespace()
	}

	pub fn track_name(&self) -> &str {
		self.subscribe.track_name()
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
