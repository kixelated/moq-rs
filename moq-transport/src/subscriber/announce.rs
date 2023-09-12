use std::collections::{hash_map, HashMap, VecDeque};

use super::Session;
use crate::{message, Error, Watch};

#[derive(Debug)]
pub(crate) struct Announces {
	lookup: HashMap<String, AnnounceRemote>,
	queue: VecDeque<Announce>,
	closed: Result<(), Error>,
}

impl Announces {
	pub fn new() -> Self {
		Self {
			lookup: HashMap::new(),
			queue: VecDeque::new(),
			closed: Ok(()),
		}
	}

	pub fn insert(&mut self, announce: Announce) -> Result<(), Error> {
		self.closed?;

		let recv = AnnounceRemote::new(&announce);

		match self.lookup.entry(announce.namespace().to_string()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(recv),
		};

		self.queue.push_back(announce);

		Ok(())
	}

	pub fn has_next(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(!self.queue.is_empty())
	}

	pub fn next(&mut self) -> Result<Announce, Error> {
		self.closed?;

		// We intentionally panic here because it's easy to write an infinite loop of wakeups if you return an Option.
		// Using lock_mut() but NOT causing a state change will still cause a Notify, which can cause a wakeup.
		Ok(self.queue.pop_front().expect("queue is empty"))
	}

	pub fn reset(&mut self, msg: &message::AnnounceReset) -> Result<(), Error> {
		self.closed?;

		let announce = self.lookup.get(&msg.namespace).ok_or(Error::NotFound)?;
		announce.reset(msg.code)?;

		Ok(())
	}

	pub fn remove(&mut self, namespace: &str) -> Result<(), Error> {
		self.closed?;
		self.lookup.remove(namespace).ok_or(Error::NotFound)?;

		Ok(())
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

		if self.acked {
			return Err(Error::Duplicate);
		}

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

#[derive(Clone, Debug)]
pub struct Announce {
	state: Watch<AnnounceState>,
	parent: Session,
	namespace: String,
}

impl Announce {
	pub fn new(parent: Session, namespace: &str) -> Self {
		Self {
			state: Watch::new(AnnounceState::new()),
			parent,
			namespace: namespace.to_string(),
		}
	}

	pub fn ok(&mut self) -> Result<(), Error> {
		self.state.lock_mut().ok()?;

		self.parent.send(message::AnnounceOk {
			namespace: self.namespace.clone(),
		})?;

		Ok(())
	}

	pub fn stop(mut self, code: u32) -> Result<(), Error> {
		self.close(Error::Stop(code))
	}

	fn close(&mut self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)?;

		self.parent.send(message::AnnounceStop {
			namespace: self.namespace.clone(),
			code: err.code(),
			reason: err.reason().to_string(),
		})?;

		Ok(())
	}

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

#[derive(Debug)]
pub(crate) struct AnnounceRemote {
	state: Watch<AnnounceState>,
}

impl AnnounceRemote {
	pub fn new(announce: &Announce) -> Self {
		Self {
			state: announce.state.clone(),
		}
	}

	pub fn reset(&self, code: u32) -> Result<(), Error> {
		let err = Error::Reset(code);
		self.state.lock_mut().close(err)?;

		Ok(())
	}
}
