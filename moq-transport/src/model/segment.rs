use std::{ops::Deref, sync::Arc, time};

use crate::{Error, VarInt};
use bytes::Bytes;

use super::Watch;

pub type Segment = (Publisher, Subscriber);

pub fn new(info: Info) -> Segment {
	let state = Watch::new(State::default());
	let info = Arc::new(info);

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

// Static information about the segment.
#[derive(Debug)]
pub struct Info {
	// The sequence number of the segment within the track.
	pub sequence: VarInt,

	// The priority of the segment within the BROADCAST.
	pub priority: i32,

	// Cache the segment for at most this long.
	pub expires: Option<time::Duration>,
}

#[derive(Debug)]
struct State {
	// The data that has been received thus far.
	data: Vec<Bytes>,

	// Set when the publisher is dropped.
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
			data: Vec::new(),
			closed: Ok(()),
		}
	}
}

#[derive(Debug, Clone)]
pub struct Publisher {
	// Mutable segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// Closes the segment when all Publishers are dropped.
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	pub fn write_chunk(&mut self, data: Bytes) -> Result<(), Error> {
		let mut state = self.state.lock_mut();
		state.closed?;
		state.data.push(data);
		Ok(())
	}

	pub fn close(self, err: Error) -> Result<(), Error> {
		self.state.lock_mut().close(err)
	}
}

impl Deref for Publisher {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

#[derive(Clone, Debug)]
pub struct Subscriber {
	// Modify the segment state.
	state: Watch<State>,

	// Immutable segment state.
	info: Arc<Info>,

	// The number of chunks that we've read.
	// NOTE: Cloned subscribers inherit this index, but then run in parallel.
	index: usize,

	// Dropped when all Subscribers are dropped.
	_dropped: Arc<Dropped>,
}

impl Subscriber {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));

		Self {
			state,
			info,
			index: 0,
			_dropped,
		}
	}

	pub async fn read_chunk(&mut self) -> Result<Option<Bytes>, Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.index < state.data.len() {
					let chunk = state.data[self.index].clone();
					self.index += 1;
					return Ok(Some(chunk));
				}

				match state.closed {
					Err(Error::Closed) => return Ok(None),
					Err(err) => return Err(err),
					Ok(()) => state.changed(),
				}
			};

			notify.await; // Try again when the state changes
		}
	}
}

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
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
