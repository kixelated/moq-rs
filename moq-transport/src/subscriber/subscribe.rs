use std::collections::{hash_map, HashMap, VecDeque};

use webtransport_quinn::RecvStream;

use super::Session;
use crate::{message, Error, Object, VarInt, Watch};

#[derive(Debug)]
pub(crate) struct Subscribes {
	lookup: HashMap<VarInt, SubscribeRemote>,
	closed: Result<(), Error>,
}

impl Subscribes {
	pub fn new() -> Self {
		Self {
			lookup: HashMap::new(),
			closed: Ok(()),
		}
	}

	pub fn insert(&mut self, subscribe: &Subscribe) -> Result<(), Error> {
		self.closed?;

		let subscribe = SubscribeRemote::new(subscribe);
		match self.lookup.entry(subscribe.id()) {
			hash_map::Entry::Occupied(_) => return Err(Error::Duplicate),
			hash_map::Entry::Vacant(v) => v.insert(subscribe),
		};

		Ok(())
	}

	pub fn ok(&mut self, msg: &message::SubscribeOk) -> Result<(), Error> {
		self.closed?;

		let subscribe = self.lookup.get_mut(&msg.id).ok_or(Error::NotFound)?;
		subscribe.ok()?;

		Ok(())
	}

	pub fn stop(&mut self, msg: &message::SubscribeStop) -> Result<(), Error> {
		self.closed?;

		let subscribe = self.lookup.get_mut(&msg.id).ok_or(Error::NotFound)?;
		subscribe.stop(msg.code, &msg.reason)?;

		Ok(())
	}

	pub fn object(&mut self, obj: &message::Object, stream: RecvStream) -> Result<(), Error> {
		self.closed?;

		let subscribe = self.lookup.get_mut(&obj.track).ok_or(Error::NotFound)?;

		let header = Object {
			sequence: obj.sequence,
			send_order: obj.send_order,
		};

		subscribe.object(header, stream)?;

		Ok(())
	}

	pub fn remove(&mut self, id: VarInt) -> Result<(), Error> {
		self.lookup.remove(&id).ok_or(Error::NotFound)?;
		Ok(())
	}

	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);

		Ok(())
	}
}
#[derive(Debug)]
struct SubscribeState {
	acked: bool,
	closed: Result<(), Error>,
	queue: VecDeque<(Object, RecvStream)>,
}

impl SubscribeState {
	pub fn new() -> Self {
		Self {
			acked: false,
			closed: Ok(()),
			queue: VecDeque::new(),
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

	pub fn object(&mut self, header: Object, stream: RecvStream) -> Result<(), Error> {
		self.closed?;
		self.queue.push_back((header, stream));
		Ok(())
	}

	pub fn has_next(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(!self.queue.is_empty())
	}

	pub fn next(&mut self) -> Result<(Object, RecvStream), Error> {
		self.closed?;

		// We intentionally panic here because it's easy to write an infinite loop of wakeups if you return an Option.
		// Using lock_mut() but NOT causing a state change will still cause a Notify, which can cause a wakeup.
		Ok(self.queue.pop_front().expect("queue is empty"))
	}

	pub fn acked(&self) -> Result<bool, Error> {
		self.closed?;
		Ok(self.acked)
	}
}

#[derive(Clone, Debug)]
pub struct Subscribe {
	state: Watch<SubscribeState>,
	parent: Session,
	id: VarInt,
	namespace: String,
	name: String,
}

impl Subscribe {
	pub fn new(parent: Session, id: VarInt, namespace: &str, name: &str) -> Self {
		Self {
			state: Watch::new(SubscribeState::new()),
			parent,
			id,
			namespace: namespace.to_string(),
			name: name.to_string(),
		}
	}

	pub async fn object(&mut self) -> Result<(Object, RecvStream), Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				if state.has_next()? {
					// Upgrade to a mutable lock once we know we can read.
					let stream = state.as_mut().next()?;
					return Ok(stream);
				}

				// Otherwise return the notify
				state.changed()
			};

			notify.await;
		}
	}

	pub fn reset(mut self, code: u32, _reason: &str) -> Result<(), Error> {
		self.close(Error::Reset(code))
	}

	fn close(&mut self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)?;

		self.parent.send(message::SubscribeReset {
			id: self.id,
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

			notify.await;
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

			notify.await;
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

#[derive(Debug)]
pub(crate) struct SubscribeRemote {
	state: Watch<SubscribeState>,
	parent: Session,
	id: VarInt,
}

impl SubscribeRemote {
	pub fn new(subscribe: &Subscribe) -> Self {
		Self {
			state: subscribe.state.clone(),
			parent: subscribe.parent.clone(),
			id: subscribe.id,
		}
	}

	pub fn id(&self) -> VarInt {
		self.id
	}

	pub fn ok(&mut self) -> Result<(), Error> {
		self.state.lock_mut().ok()?;

		Ok(())
	}

	pub fn stop(&mut self, code: u32, _reason: &str) -> Result<(), Error> {
		let err = Error::Stop(code);
		self.state.lock_mut().close(err)?;

		self.parent.send(message::SubscribeReset {
			id: self.id,
			code: err.code(),
			reason: err.reason().to_string(),
		})?;

		Ok(())
	}

	pub fn object(&mut self, header: Object, stream: RecvStream) -> Result<(), Error> {
		self.state.lock_mut().object(header, stream)?;
		Ok(())
	}
}
