use std::{collections::VecDeque, ops};

use crate::watch::State;
use crate::{message, serve::ServeError};

use super::{Publisher, Subscribed};

#[derive(Debug, Clone)]
pub struct AnnounceInfo {
	pub namespace: String,
}

struct AnnounceState {
	subscribers: VecDeque<Subscribed>,
	ok: bool,
	closed: Result<(), ServeError>,
}

impl Default for AnnounceState {
	fn default() -> Self {
		Self {
			subscribers: Default::default(),
			ok: false,
			closed: Ok(()),
		}
	}
}

impl Drop for AnnounceState {
	fn drop(&mut self) {
		for subscriber in self.subscribers.drain(..) {
			subscriber.close(ServeError::NotFound).ok();
		}
	}
}

#[must_use = "unannounce on drop"]
pub struct Announce {
	publisher: Publisher,
	state: State<AnnounceState>,

	pub info: AnnounceInfo,
}

impl Announce {
	pub(super) fn new(mut publisher: Publisher, namespace: String) -> (Announce, AnnounceRecv) {
		let info = AnnounceInfo {
			namespace: namespace.clone(),
		};

		publisher.send_message(message::Announce {
			namespace,
			params: Default::default(),
		});

		let (send, recv) = State::default().split();

		let send = Self {
			publisher,
			info,
			state: send,
		};
		let recv = AnnounceRecv { state: recv };

		(send, recv)
	}

	// Run until we get an error
	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				state.closed.clone()?;

				match state.modified() {
					Some(notified) => notified,
					None => return Ok(()),
				}
			}
			.await;
		}
	}

	pub async fn subscribed(&mut self) -> Result<Option<Subscribed>, ServeError> {
		loop {
			{
				let state = self.state.lock();
				if !state.subscribers.is_empty() {
					return Ok(state.into_mut().and_then(|mut state| state.subscribers.pop_front()));
				}

				state.closed.clone()?;
				match state.modified() {
					Some(notified) => notified,
					None => return Ok(None),
				}
			}
			.await;
		}
	}

	// Wait until an OK is received
	pub async fn ok(&self) -> Result<(), ServeError> {
		loop {
			{
				let state = self.state.lock();
				if state.ok {
					return Ok(());
				}
				state.closed.clone()?;

				match state.modified() {
					Some(notified) => notified,
					None => return Ok(()),
				}
			}
			.await;
		}
	}
}

impl Drop for Announce {
	fn drop(&mut self) {
		if self.state.lock().closed.is_err() {
			return;
		}

		self.publisher.send_message(message::Unannounce {
			namespace: self.namespace.to_string(),
		});
	}
}

impl ops::Deref for Announce {
	type Target = AnnounceInfo;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub(super) struct AnnounceRecv {
	state: State<AnnounceState>,
}

impl AnnounceRecv {
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

	pub fn recv_subscribe(&mut self, subscriber: Subscribed) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut().ok_or(ServeError::Done)?;
		state.subscribers.push_back(subscriber);

		Ok(())
	}
}
