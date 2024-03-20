use crate::{control, serve::ServeError, util::Watch};

use super::Publisher;

pub struct Announce {
	session: Publisher,
	namespace: String,
	state: Watch<State>,
}

impl Announce {
	pub(super) fn new(session: Publisher, namespace: String) -> (Announce, AnnounceRecv) {
		let state = Watch::default();
		let recv = AnnounceRecv { state: state.clone() };

		let announce = Self {
			session,
			namespace,
			state,
		};

		(announce, recv)
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	fn close(&mut self) -> Result<(), ServeError> {
		let mut state = self.state.lock_mut();
		state.closed.clone()?;
		state.closed = Err(ServeError::Done);

		self.session
			.send_message(control::Unannounce {
				namespace: self.namespace.clone(),
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

impl Drop for Announce {
	fn drop(&mut self) {
		self.close().ok();
		self.session.drop_announce(&self.namespace);
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
