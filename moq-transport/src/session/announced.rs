use crate::{message, serve::ServeError, util::State};

use super::Subscriber;

// There's currently no feedback from the peer, so the shared state is empty.
// If Unannounce contained an error code then we'd be talking.
#[derive(Default)]
struct AnnouncedState {}

pub struct Announced<S: webtransport_generic::Session> {
	session: Subscriber<S>,
	namespace: String,
	state: State<AnnouncedState>,

	ok: bool,
	error: Option<ServeError>,
}

impl<S: webtransport_generic::Session> Announced<S> {
	pub(super) fn new(session: Subscriber<S>, namespace: String) -> (Announced<S>, AnnouncedRecv) {
		let (send, recv) = State::default();
		let send = Self {
			session,
			namespace,
			ok: false,
			error: None,
			state: send,
		};
		let recv = AnnouncedRecv { _state: recv };

		(send, recv)
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	// Send an ANNOUNCE_OK and block until we get an UNANNOUNCE
	pub async fn serve(mut self) -> Result<(), ServeError> {
		self.session.send_message(message::AnnounceOk {
			namespace: self.namespace.clone(),
		});

		self.ok = true;

		loop {
			// Wow this is dumb and yet pretty cool.
			// Basically loop until the state changes and exit when Recv is dropped.
			self.state.lock().modified().ok_or(ServeError::Done)?.await;
		}
	}

	pub fn close(mut self, err: ServeError) -> Result<(), ServeError> {
		self.error = Some(err);
		Ok(())
	}
}

impl<S: webtransport_generic::Session> Drop for Announced<S> {
	fn drop(&mut self) {
		let err = self.error.clone().unwrap_or(ServeError::Done);

		if self.ok {
			self.session.send_message(message::AnnounceCancel {
				namespace: self.namespace.clone(),
			});
		} else {
			self.session.send_message(message::AnnounceError {
				namespace: self.namespace.clone(),
				code: err.code(),
				reason: err.to_string(),
			});
		}
	}
}

pub(super) struct AnnouncedRecv {
	_state: State<AnnouncedState>,
}

impl AnnouncedRecv {
	pub fn recv_unannounce(self) -> Result<(), ServeError> {
		// Will cause the state to be dropped
		Ok(())
	}
}
