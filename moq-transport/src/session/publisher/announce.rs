use std::sync::{Arc, Mutex, Weak};

use crate::{control, error::AnnounceError};

use super::Publisher;

#[derive(Clone)]
pub struct Announce {
	namespace: String,
	state: Arc<Mutex<AnnounceState>>,
}

impl Announce {
	pub(super) fn new(session: Publisher, namespace: String) -> Self {
		let state = Arc::new(Mutex::new(AnnounceState::new(session, namespace.clone())));
		Self { namespace, state }
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}

	pub(super) fn close(&mut self, err: AnnounceError) -> Result<(), AnnounceError> {
		self.state.lock().unwrap().close(err)
	}

	pub async fn closed(&self) -> Result<(), AnnounceError> {
		self.state.lock().unwrap().closed.clone()
	}

	pub(super) fn downgrade(&self) -> AnnounceWeak {
		AnnounceWeak {
			namespace: self.namespace.clone(),
			state: Arc::downgrade(&self.state),
		}
	}
}

pub(super) struct AnnounceWeak {
	namespace: String,
	state: Weak<Mutex<AnnounceState>>,
}

impl AnnounceWeak {
	pub fn upgrade(&self) -> Option<Announce> {
		Some(Announce {
			state: self.state.upgrade()?,
			namespace: self.namespace.clone(),
		})
	}
}

pub(super) struct AnnounceState {
	session: Publisher,
	namespace: String,
	closed: Result<(), AnnounceError>,
}

impl AnnounceState {
	pub fn new(session: Publisher, namespace: String) -> Self {
		Self {
			session,
			namespace,
			closed: Ok(()),
		}
	}

	pub fn close(&mut self, err: AnnounceError) -> Result<(), AnnounceError> {
		self.closed.clone()?;
		self.closed = Err(err.clone());

		self.session.send_message(control::Unannounce {
			namespace: self.namespace.clone(),
		})?;

		Ok(())
	}
}

impl Drop for AnnounceState {
	fn drop(&mut self) {
		self.close(AnnounceError::Done).unwrap();
		self.session.drop_announce(&self.namespace);
	}
}
