use std::collections::{hash_map, HashMap};

use super::Session;
use crate::{message, Error, Watch};

#[derive(Debug)]
pub(crate) struct Announces {
	lookup: HashMap<String, AnnounceRemote>,
	closed: Result<(), Error>,
}

impl Announces {
	pub fn new() -> Self {
		Self {
			lookup: HashMap::new(),
			closed: Ok(()),
		}
	}

	pub fn insert(&mut self, announce: &Announce) -> Result<(), Error> {
		self.closed?;
		let announce = AnnounceRemote::new(announce);

		match self.lookup.entry(announce.namespace().to_string()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(entry) => entry.insert(announce),
		};

		Ok(())
	}

	pub fn remove(&mut self, namespace: &str) -> Result<(), Error> {
		self.closed?;
		self.lookup.remove(namespace).ok_or(Error::NotFound)?;
		Ok(())
	}

	pub fn ok(&mut self, msg: &message::AnnounceOk) -> Result<(), Error> {
		self.closed?;
		let announce = self.lookup.get_mut(&msg.namespace).ok_or(Error::NotFound)?;
		announce.ok()
	}

	pub fn stop(&mut self, msg: &message::AnnounceStop) -> Result<(), Error> {
		self.closed?;
		let announce = self.lookup.get_mut(&msg.namespace).ok_or(Error::NotFound)?;
		announce.stop(msg.code, &msg.reason)
	}

	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);
		Ok(())
	}
}

#[derive(Debug)]
struct AnnounceState {
	acked: bool,
	closed: Result<(), Error>,
}

impl AnnounceState {
	pub fn new() -> Self {
		Self {
			acked: false,
			closed: Ok(()),
		}
	}

	pub fn ok(&mut self) -> Result<(), Error> {
		self.closed?;
		self.acked = true;

		Ok(())
	}

	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);

		Ok(())
	}

	pub fn acked(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(self.acked)
	}
}

// The public API.
#[derive(Clone, Debug)]
pub struct Announce {
	state: Watch<AnnounceState>,
	parent: Session,
	namespace: String,
}

impl Announce {
	pub(crate) fn new(parent: Session, namespace: &str) -> Self {
		let state = Watch::new(AnnounceState::new());

		Self {
			state,
			parent,
			namespace: namespace.to_string(),
		}
	}

	pub fn reset(mut self, code: u32) -> Result<(), Error> {
		self.close(Error::Reset(code))
	}

	fn close(&mut self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)?;

		self.parent.send(message::AnnounceReset {
			namespace: self.namespace.clone(),
			code: err.code(),
			reason: err.reason().to_string(),
		})?;

		Ok(())
	}

	// Block until the announce has been acknowledged.
	pub async fn acked(&self) -> Result<(), Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.acked()? {
					return Ok(());
				}

				state.changed()
			};

			notify.await; // Try again when the state changes
		}
	}

	/// Block while the announcement is active.
	pub async fn active(&self) -> Result<(), Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				state.closed?;
				state.changed()
			};

			notify.await; // try again when the state changes
		}
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}
}

impl Drop for Announce {
	fn drop(&mut self) {
		self.close(Error::Closed).ok();

		// Drop the AnnounceRemote
		self.parent.unannounce(&self.namespace).ok();
	}
}

// A handle for the Session to process incoming messages.
#[derive(Debug)]
pub(crate) struct AnnounceRemote {
	state: Watch<AnnounceState>,
	parent: Session,
	namespace: String,
}

impl AnnounceRemote {
	// Make a receive handle for the Session to process incoming messages.
	pub fn new(other: &Announce) -> AnnounceRemote {
		AnnounceRemote {
			state: other.state.clone(),
			parent: other.parent.clone(),
			namespace: other.namespace.clone(),
		}
	}

	pub fn ok(&mut self) -> Result<(), Error> {
		self.state.lock_mut().ok()?;

		Ok(())
	}

	pub fn stop(&mut self, code: u32, _reason: &str) -> Result<(), Error> {
		let err = Error::Stop(code);
		self.state.lock_mut().close(err)?;

		self.parent.send(message::AnnounceReset {
			namespace: self.namespace.clone(),
			code: err.code(),
			reason: err.reason().to_string(),
		})?;

		Ok(())
	}

	pub fn namespace(&self) -> &str {
		&self.namespace
	}
}
