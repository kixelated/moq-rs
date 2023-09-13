//! A broker is a list of broadcasts, split into a [Publisher] and [Subscriber] handle.
//!
//! The [Publisher] can be used to insert and remove broadcasts.
//! These coorespond to ANNOUNCE messages in the protocol, signalling that a new broadcast is available.
//!
//! The [Subscriber] can be used to receive a copy of all new broadcasts.
//! A cloned [Subscriber] will receive a copy of all new broadcasts going forward (fanout).
//!
//! The broker is closed with [Error::Closed] when all publishers or subscribers are dropped.
use std::{fmt, sync::Arc};

use indexmap::IndexMap;

use crate::Error;

use super::{broadcast, Watch};

/// Create a broker that can be used to publish and subscribe a list of broadcasts.
pub fn new() -> (Publisher, Subscriber) {
	let state = Watch::new(State::default());

	let publisher = Publisher::new(state.clone());
	let subscriber = Subscriber::new(state);

	(publisher, subscriber)
}

#[derive(Debug)]
struct State {
	// This is a HashMap that keeps track of insertion order.
	// We replace the entry when None when it's removed, otherwise subscribers would get confused.
	// We remove any None entries at the start of the map just to keep growth in check.
	lookup: IndexMap<String, Option<broadcast::Subscriber>>,

	// Incremented by one each time we remove the first element.
	pruned: usize,

	// Closed when the publisher or all subscribers are dropped.
	closed: Result<(), Error>,
}

impl State {
	pub fn close(&mut self, err: Error) -> Result<(), Error> {
		self.closed?;
		self.closed = Err(err);
		Ok(())
	}
}

impl Default for State {
	fn default() -> Self {
		Self {
			lookup: IndexMap::new(),
			pruned: 0,
			closed: Ok(()),
		}
	}
}

/// Publish new broadcasts by name.
#[derive(Clone)]
pub struct Publisher {
	state: Watch<State>,

	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, _dropped }
	}

	/// Insert a broadcast into the list.
	pub fn insert_broadcast(&mut self, broadcast: broadcast::Subscriber) -> Result<(), Error> {
		let state = self.state.lock();
		state.closed?;

		match state.into_mut().lookup.entry(broadcast.name.clone()) {
			indexmap::map::Entry::Occupied(_) => return Err(Error::Duplicate),
			indexmap::map::Entry::Vacant(v) => v.insert(Some(broadcast)),
		};

		Ok(())
	}

	/// Create and insert a broadcast into the list.
	pub fn create_broadcast(&mut self, name: &str) -> Result<(broadcast::Publisher, broadcast::Unknown), Error> {
		let (publisher, subscriber, unknown) = broadcast::new(name);
		self.insert_broadcast(subscriber)?;
		Ok((publisher, unknown))
	}

	/// Remove a broadcast from the list.
	pub fn remove_broadcast(&mut self, name: &str) -> Result<(), Error> {
		let mut state = self.state.lock_mut();

		match state.lookup.entry(name.to_string()) {
			indexmap::map::Entry::Occupied(mut entry) => entry.insert(None),
			indexmap::map::Entry::Vacant(_) => return Err(Error::NotFound),
		};

		// Remove None entries from the start of the lookup.
		while let Some((_, None)) = state.lookup.get_index(0) {
			state.lookup.shift_remove_index(0);
			state.pruned += 1;
		}

		Ok(())
	}
}

impl fmt::Debug for Publisher {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Publisher").field("state", &self.state).finish()
	}
}

/// Receives a notification for all new broadcasts.
///
/// This can be cloned to create handles, each receiving a copy of all broadcasts going forward.
#[derive(Clone)]
pub struct Subscriber {
	state: Watch<State>,
	index: usize,

	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self {
			state,
			index: 0,
			_dropped,
		}
	}

	/// Get a broadcast by name, assuming it has been already inserted.
	pub fn get_broadcast(&mut self, name: &str) -> Result<broadcast::Subscriber, Error> {
		let state = self.state.lock();
		if let Some(Some(subscriber)) = state.lookup.get(name) {
			Ok(subscriber.clone())
		} else {
			Err(Error::NotFound)
		}
	}

	/// Block until the next broadcast is announced.
	pub async fn next_broadcast(&mut self) -> Result<Option<broadcast::Subscriber>, Error> {
		loop {
			let notify = {
				let state = self.state.lock();

				loop {
					// Get our adjusted index, which could be negative if we've removed more broadcasts than read.
					let index = self.index.saturating_sub(state.pruned);
					if index >= state.lookup.len() {
						break;
					}

					self.index = index + state.pruned + 1;

					let next = state.lookup.get_index(index).unwrap();
					if let Some(next) = next.1 {
						return Ok(Some(next.clone()));
					}
				}

				match state.closed {
					Err(Error::Closed) => return Ok(None),
					Err(err) => return Err(err),
					Ok(()) => state.changed(),
				}
			};

			notify.await;
		}
	}
}

impl fmt::Debug for Subscriber {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Subscriber")
			.field("state", &self.state)
			.field("index", &self.index)
			.finish()
	}
}

struct Dropped {
	// Modify the segment state.
	state: Watch<State>,
}

impl Dropped {
	fn new(state: Watch<State>) -> Self {
		Self { state }
	}
}

impl Drop for Dropped {
	fn drop(&mut self) {
		self.state.lock_mut().close(Error::Closed).ok();
	}
}
