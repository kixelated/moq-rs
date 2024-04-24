use std::ops;

use crate::watch::State;
use crate::{message, serve::ServeError};

use super::{AnnounceInfo, Subscriber};

// There's currently no feedback from the peer, so the shared state is empty.
// If Unannounce contained an error code then we'd be talking.
#[derive(Default)]
struct AnnouncedState {}

pub struct Announced {
	session: Subscriber,
	state: State<AnnouncedState>,

	pub info: AnnounceInfo,

	ok: bool,
	error: Option<ServeError>,
}

impl Announced {
	pub(super) fn new(session: Subscriber, namespace: String) -> (Announced, AnnouncedRecv) {
		let info = AnnounceInfo { namespace };

		let (send, recv) = State::default().split();
		let send = Self {
			session,
			info,
			ok: false,
			error: None,
			state: send,
		};
		let recv = AnnouncedRecv { _state: recv };

		(send, recv)
	}

	// Send an ANNOUNCE_OK
	pub fn ok(&mut self) -> Result<(), ServeError> {
		if self.ok {
			return Err(ServeError::Duplicate);
		}

		self.session.send_message(message::AnnounceOk {
			namespace: self.namespace.clone(),
		});

		self.ok = true;

		Ok(())
	}

	pub async fn closed(&self) -> Result<(), ServeError> {
		loop {
			// Wow this is dumb and yet pretty cool.
			// Basically loop until the state changes and exit when Recv is dropped.
			self.state.lock().modified().ok_or(ServeError::Cancel)?.await;
		}
	}

	pub fn close(mut self, err: ServeError) -> Result<(), ServeError> {
		self.error = Some(err);
		Ok(())
	}
}

impl ops::Deref for Announced {
	type Target = AnnounceInfo;

	fn deref(&self) -> &AnnounceInfo {
		&self.info
	}
}

impl Drop for Announced {
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
