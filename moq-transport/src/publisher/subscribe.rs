use std::collections::{hash_map, HashMap, VecDeque};

use webtransport_quinn::SendStream;

use super::Session;
use crate::{message, Error, Object, VarInt, Watch};

#[derive(Debug)]
pub(crate) struct Subscribes {
	lookup: HashMap<VarInt, SubscribeRemote>,
	queue: VecDeque<Subscribe>,
	closed: Result<(), Error>,
}

impl Subscribes {
	pub fn new() -> Self {
		Self {
			lookup: HashMap::new(),
			queue: VecDeque::new(),
			closed: Ok(()),
		}
	}

	pub fn insert(&mut self, id: VarInt, sub: Subscribe) -> Result<(), Error> {
		self.closed?;

		let remote = SubscribeRemote::new(&sub);

		match self.lookup.entry(id) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(entry) => entry.insert(remote),
		};

		self.queue.push_back(sub);

		Ok(())
	}

	pub fn reset(&mut self, msg: &message::SubscribeReset) -> Result<(), Error> {
		self.closed?;

		let subscribe = self.lookup.get_mut(&msg.id).ok_or(Error::NotFound)?;
		subscribe.reset(msg.code, &msg.reason)?;

		Ok(())
	}

	pub fn remove(&mut self, id: VarInt) -> Result<(), Error> {
		self.closed?;

		self.lookup.remove(&id).ok_or(Error::NotFound)?;
		Ok(())
	}

	pub fn has_next(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(!self.queue.is_empty())
	}

	pub fn next(&mut self) -> Result<Subscribe, Error> {
		self.closed?;

		// We intentionally panic here because it's easy to write an infinite loop of wakeups if you return an Option.
		// Using lock_mut() but NOT causing a state change will still cause a Notify, which can cause a wakeup.
		Ok(self.queue.pop_front().expect("queue is empty"))
	}

	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);
		Ok(())
	}
}

// All mutable state goes here.
#[derive(Debug)]
struct SubscribeState {
	acked: bool,
	closed: Result<(), Error>,
}

impl SubscribeState {
	pub fn new() -> Self {
		Self {
			acked: false,
			closed: Ok(()),
		}
	}

	pub fn acked(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(self.acked)
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
}

// Public methods go here.
#[derive(Clone, Debug)]
pub struct Subscribe {
	state: Watch<SubscribeState>,
	parent: Session,
	namespace: String,
	name: String,
	id: VarInt,
}

impl Subscribe {
	pub(crate) fn new(parent: Session, namespace: &str, name: &str, id: VarInt) -> Self {
		let state = Watch::new(SubscribeState::new());

		Self {
			state,
			parent,
			namespace: namespace.to_string(),
			name: name.to_string(),
			id,
		}
	}

	/// Create a new stream.
	pub async fn object(&mut self, header: Object) -> Result<SendStream, Error> {
		let obj = message::Object {
			track: self.id,
			group: header.sequence, // TODO verify it's not a duplicate
			sequence: VarInt::from_u32(0),
			send_order: header.send_order,
		};

		log::debug!("sending object: {:?}", obj);

		let mut stream = self
			.parent
			.webtransport()
			.open_uni()
			.await
			.map_err(|_e| Error::Unknown)?;

		stream.set_priority(obj.send_order).ok();

		// TODO do this in segment for flow control reasons.
		// We might be under MAX_STREAMS but over MAX_DATA, or MAX_STREAM_DATA defaults to 0
		// TODO better handle the error.
		obj.encode(&mut stream).await.map_err(|_e| Error::Unknown)?;

		Ok(stream)
	}

	/// Send an optional OK message.
	pub fn ok(&mut self) -> Result<(), Error> {
		self.state.lock_mut().ok()?;

		self.parent.send(message::SubscribeOk {
			id: self.id,
			expires: None,
		})?;

		Ok(())
	}

	fn close(&mut self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)?;

		self.parent.send(message::SubscribeStop {
			id: self.id,
			code: err.code(),
			reason: err.reason().to_string(),
		})?;

		Ok(())
	}

	pub fn stop(mut self, code: u32, _reason: &str) -> Result<(), Error> {
		self.close(Error::Stop(code))
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

	/// Block while the subscription is active.
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

	pub fn name(&self) -> &str {
		&self.name
	}
}

impl Drop for Subscribe {
	fn drop(&mut self) {
		self.close(Error::Closed).ok();

		// Drop the SubscribeRemote
		self.parent.unsubscribe(self.id).ok();
	}
}

// Private methods go here, used to process incoming messages
#[derive(Debug)]
pub(crate) struct SubscribeRemote {
	state: Watch<SubscribeState>,
}

impl SubscribeRemote {
	pub fn new(other: &Subscribe) -> Self {
		Self {
			state: other.state.clone(),
		}
	}

	pub fn reset(&mut self, code: u32, _reason: &str) -> Result<(), Error> {
		let err = Error::Reset(code);
		self.state.lock_mut().close(err)?;
		Ok(())
	}
}
