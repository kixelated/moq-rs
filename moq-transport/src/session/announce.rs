use crate::{message, serve::ServeError, util::Watch};

use super::Publisher;

pub struct Announce<S: webtransport_generic::Session> {
	session: Publisher<S>,
	msg: message::Announce,
	state: Watch<State>,
}

impl<S: webtransport_generic::Session> Announce<S> {
	pub(super) fn new(session: Publisher<S>, msg: message::Announce) -> (Announce<S>, AnnounceRecv) {
		let state = Watch::default();
		let recv = AnnounceRecv { state: state.clone() };

		let announce = Self { session, msg, state };

		(announce, recv)
	}

	pub fn namespace(&self) -> &str {
		&self.msg.namespace
	}

	fn close(&mut self) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.closed = Err(ServeError::Done);

		self.session
			.send_message(message::Unannounce {
				namespace: self.msg.namespace.clone(),
			})
			.ok();

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed.clone()?;
				state.changed()
			};

			notify.await;
		}
	}
}

impl<S: webtransport_generic::Session> Drop for Announce<S> {
	fn drop(&mut self) {
		self.close().ok();
		self.session.drop_announce(&self.msg.namespace);
	}
}

pub(super) struct AnnounceRecv {
	state: Watch<State>,
}

impl AnnounceRecv {
	pub fn recv_error(&mut self, err: ServeError) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.closed = Err(err);
		Ok(())
	}
}

struct State {
	closed: Result<(), ServeError>,
}

impl Default for State {
	fn default() -> Self {
		Self { closed: Ok(()) }
	}
}
