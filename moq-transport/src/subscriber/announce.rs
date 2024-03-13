use std::sync::{Arc, Mutex, Weak};

use crate::{control, error::AnnounceError};

use super::Session;

#[derive(Clone)]
pub struct Announce {
	namespace: String,
	state: Arc<Mutex<AnnounceState>>,
}

impl Announce {
	pub(super) fn new(session: Session, namespace: String) -> Self {
		let state = Arc::new(Mutex::new(AnnounceState::new(session, namespace.clone())));
		Self { namespace, state }
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	pub async fn closed(&self) -> Result<(), AnnounceError> {
		self.state.lock().unwrap().closed.clone()
	}

	pub(super) fn downgrade(&self) -> AnnounceWeak {
		AnnounceWeak {
			state: Arc::downgrade(&self.state),
		}
	}
}

pub(super) struct AnnounceWeak {
	state: Weak<Mutex<AnnounceState>>,
}

impl AnnounceWeak {
	pub fn close(&mut self, err: AnnounceError) -> Result<(), AnnounceError> {
		if let Some(state) = self.state.upgrade() {
			state.lock().unwrap().close(err)
		} else {
			Err(AnnounceError::Dropped)
		}
	}
}

pub(super) struct AnnounceState {
	session: Session,
	namespace: String,

	ok: bool,
	closed: Result<(), AnnounceError>,
}

impl AnnounceState {
	pub fn new(session: Session, namespace: String) -> Self {
		Self {
			session,
			namespace,
			ok: false,
			closed: Ok(()),
		}
	}

	pub fn ok(&mut self) -> Result<(), AnnounceError> {
		self.closed.clone()?;
		self.ok = true;
		Ok(())
	}

	pub fn close(&mut self, err: AnnounceError) -> Result<(), AnnounceError> {
		self.closed.clone()?;
		self.closed = Err(err.clone());
		self.session.remove_announce(self.namespace.clone());

		if self.ok {
			self.session.send_message(control::AnnounceCancel {
				namespace: self.namespace.clone(),
			})?;
		} else {
			self.session.send_message(control::AnnounceError {
				namespace: self.namespace.clone(),
				code: err.code().into(),
				reason: err.to_string(),
			})?;
		}

		Ok(())
	}
}

impl Drop for AnnounceState {
	fn drop(&mut self) {
		self.close(AnnounceError::Done).unwrap();
	}
}

pub struct AnnouncePending {
	announce: Announce,
}

impl AnnouncePending {
	pub(crate) fn new(announce: Announce) -> Self {
		Self { announce }
	}

	pub fn namespace(&self) -> &str {
		self.announce.namespace()
	}

	pub fn accept(self) -> Result<Announce, AnnounceError> {
		self.announce.state.lock().unwrap().ok()?;
		Ok(self.announce)
	}

	pub fn reject(self, err: AnnounceError) -> Result<(), AnnounceError> {
		self.announce.state.lock().unwrap().close(err)?;
		Ok(())
	}
}
