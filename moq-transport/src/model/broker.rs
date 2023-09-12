use std::sync::Arc;

use indexmap::IndexMap;

use crate::Error;

use super::{broadcast, Watch};

pub type Broker = (Publisher, Subscriber);

pub fn new() -> Broker {
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

#[derive(Debug, Clone)]
pub struct Publisher {
	state: Watch<State>,

	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, _dropped }
	}

	pub fn insert_broadcast(&mut self, broadcast: broadcast::Subscriber) -> Result<(), Error> {
		let state = self.state.lock();
		state.closed?;

		match state.as_mut().lookup.entry(broadcast.name.clone()) {
			indexmap::map::Entry::Occupied(_) => return Err(Error::Duplicate),
			indexmap::map::Entry::Vacant(v) => v.insert(Some(broadcast)),
		};

		Ok(())
	}

	pub fn create_broadcast(&mut self, name: &str) -> Result<(broadcast::Publisher, broadcast::Unknown), Error> {
		let (publisher, subscriber, unknown) = broadcast::new(name);
		self.insert_broadcast(subscriber)?;
		Ok((publisher, unknown))
	}

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

#[derive(Clone, Debug)]
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

	pub fn get_broadcast(&mut self, name: &str) -> Result<broadcast::Subscriber, Error> {
		let state = self.state.lock();
		if let Some(Some(subscriber)) = state.lookup.get(name) {
			Ok(subscriber.clone())
		} else {
			Err(Error::NotFound)
		}
	}

	pub async fn next_broadcast(&mut self) -> Result<Option<broadcast::Subscriber>, Error> {
		loop {
			let notify = {
				let state = self.state.lock();

				// Get our adjusted index, which could be negative if we've removed more broadcasts than read.
				let index = self.index.saturating_sub(state.pruned);

				while index < state.lookup.len() {
					self.index += 1;

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

#[derive(Clone, Debug)]
pub struct Dropped {
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
