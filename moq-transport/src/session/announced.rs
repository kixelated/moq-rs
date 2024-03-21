use crate::{message, serve::ServeError, util::Watch};

use super::Subscriber;

pub struct Announced<S: webtransport_generic::Session> {
	session: Subscriber<S>,
	namespace: String,
	state: Watch<State<S>>,
}

impl<S: webtransport_generic::Session> Announced<S> {
	pub(super) fn new(session: Subscriber<S>, namespace: String) -> (Announced<S>, AnnouncedRecv<S>) {
		let state = Watch::new(State::new(session.clone(), namespace.clone()));
		let recv = AnnouncedRecv { state: state.clone() };

		let announced = Self {
			session,
			namespace,
			state,
		};

		(announced, recv)
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	// Send an ANNOUNCE_OK
	pub fn accept(&mut self) -> Result<(), ServeError> {
		self.state.lock_mut().accept()
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.state.lock_mut().close(err)
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

impl<S: webtransport_generic::Session> Drop for Announced<S> {
	fn drop(&mut self) {
		self.close(ServeError::Done).ok();
		self.session.drop_announce(&self.namespace);
	}
}

pub(super) struct AnnouncedRecv<S: webtransport_generic::Session> {
	state: Watch<State<S>>,
}

impl<S: webtransport_generic::Session> AnnouncedRecv<S> {
	pub fn recv_unannounce(&mut self) -> Result<(), ServeError> {
		self.state.lock_mut().close(ServeError::Done)
	}
}

struct State<S: webtransport_generic::Session> {
	namespace: String,
	session: Subscriber<S>,
	ok: bool,
	closed: Result<(), ServeError>,
}

impl<S: webtransport_generic::Session> State<S> {
	fn new(session: Subscriber<S>, namespace: String) -> Self {
		Self {
			session,
			namespace,
			ok: false,
			closed: Ok(()),
		}
	}

	pub fn accept(&mut self) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.ok = true;

		self.session
			.send_message(message::AnnounceOk {
				namespace: self.namespace.clone(),
			})
			.ok();

		Ok(())
	}

	pub fn close(&mut self, err: ServeError) -> Result<(), ServeError> {
		self.closed.clone()?;
		self.closed = Err(err.clone());

		if self.ok {
			self.session
				.send_message(message::AnnounceCancel {
					namespace: self.namespace.clone(),
				})
				.ok();
		} else {
			self.session
				.send_message(message::AnnounceError {
					namespace: self.namespace.clone(),
					code: err.code(),
					reason: err.to_string(),
				})
				.ok();
		}

		Ok(())
	}
}
