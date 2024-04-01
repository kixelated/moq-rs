use std::{collections::VecDeque, ops};

use crate::{message, serve::ServeError, Publisher};

use super::Subscribed;

use crate::util::State;

#[derive(Debug, Clone)]
pub struct AnnounceInfo {
	pub namespace: String,
}

struct AnnounceState<S: webtransport_generic::Session> {
	subscribers: VecDeque<Subscribed<S>>,
	ok: bool,
	closed: Result<(), ServeError>,
}

impl<S: webtransport_generic::Session> Default for AnnounceState<S> {
	fn default() -> Self {
		Self {
			subscribers: Default::default(),
			ok: false,
			closed: Ok(()),
		}
	}
}

impl<S: webtransport_generic::Session> Drop for AnnounceState<S> {
	fn drop(&mut self) {
		for subscriber in self.subscribers.drain(..) {
			subscriber.close(ServeError::NotFound).ok();
		}
	}
}

pub struct Announce<S: webtransport_generic::Session> {
	publisher: Publisher<S>,
	state: State<AnnounceState<S>>,

	pub info: AnnounceInfo,
}

impl<S: webtransport_generic::Session> Announce<S> {
	pub(super) fn new(mut publisher: Publisher<S>, namespace: String) -> (Announce<S>, AnnounceRecv<S>) {
		let info = AnnounceInfo {
			namespace: namespace.clone(),
		};

		publisher.send_message(message::Announce {
			namespace,
			params: Default::default(),
		});

		let (send, recv) = State::init();

		let send = Self {
			publisher,
			info,
			state: send,
		};
		let recv = AnnounceRecv { state: recv };

		(send, recv)
	}

	// Run until we get an error
	pub async fn serve(self) -> Result<(), ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notified) => notified,
					None => return Ok(()),
				}
			};

			notify.await
		}
	}

	pub async fn subscribed(&mut self) -> Result<Option<Subscribed<S>>, ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				if !state.subscribers.is_empty() {
					return Ok(state.into_mut().and_then(|mut state| state.subscribers.pop_front()));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notified) => notified,
					None => return Ok(None),
				}
			};

			notify.await
		}
	}
}

impl<S: webtransport_generic::Session> Drop for Announce<S> {
	fn drop(&mut self) {
		if self.state.lock().closed.is_err() {
			return;
		}

		self.publisher.send_message(message::Unannounce {
			namespace: self.info.namespace.to_string(),
		});
	}
}

impl<S: webtransport_generic::Session> ops::Deref for Announce<S> {
	type Target = AnnounceInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub(super) struct AnnounceRecv<S: webtransport_generic::Session> {
	state: State<AnnounceState<S>>,
}

impl<S: webtransport_generic::Session> AnnounceRecv<S> {
	pub fn recv_ok(&mut self) -> Result<(), ServeError> {
		if let Some(mut state) = self.state.lock_mut() {
			if state.ok {
				return Err(ServeError::Duplicate);
			}

			state.ok = true;
		}

		Ok(())
	}

	pub fn recv_error(self, err: ServeError) -> Result<(), ServeError> {
		let state = self.state.lock();
		state.closed.clone()?;

		let mut state = state.into_mut().ok_or(ServeError::Done)?;
		state.closed = Err(err);

		Ok(())
	}

	pub fn recv_subscribe(&mut self, subscriber: Subscribed<S>) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.subscribers.push_back(subscriber);

		Ok(())
	}
}
