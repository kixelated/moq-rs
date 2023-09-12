use std::{collections::VecDeque, ops::Deref, sync::Arc};

use super::{segment, Watch};
use crate::{Error, VarInt};

pub type Track = (Publisher, Subscriber);

pub fn new(name: &str) -> Track {
	let state = Watch::new(State::default());
	let info = Arc::new(Info { name: name.to_string() });

	let publisher = Publisher::new(state.clone(), info.clone());
	let subscriber = Subscriber::new(state, info);

	(publisher, subscriber)
}

#[derive(Debug)]
pub struct Info {
	pub name: String,
}

#[derive(Debug)]
struct State {
	segments: VecDeque<segment::Subscriber>,
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
			segments: VecDeque::new(),
			closed: Ok(()),
		}
	}
}

#[derive(Debug, Clone)]
pub struct Publisher {
	state: Watch<State>,
	info: Arc<Info>,
	_dropped: Arc<Dropped>,
}

impl Publisher {
	fn new(state: Watch<State>, info: Arc<Info>) -> Self {
		let _dropped = Arc::new(Dropped::new(state.clone()));
		Self { state, info, _dropped }
	}

	pub fn insert_segment(&mut self, segment: segment::Subscriber) -> Result<(), Error> {
		let state = self.state.lock();
		state.closed?;

		// TODO check for duplicates
		// TODO insert in (priority?) order
		state.as_mut().segments.push_back(segment);

		Ok(())
	}

	// Helper method to create and insert a segment in one step.
	pub fn create_segment(&mut self, sequence: VarInt, order: i32) -> Result<segment::Publisher, Error> {
		let (publisher, subscriber) = segment::new(sequence, order);
		self.insert_segment(subscriber)?;
		Ok(publisher)
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
	state: Watch<State>,
	info: Arc<Info>,
	index: usize,
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

	pub async fn next_segment(&mut self) -> Result<Option<segment::Subscriber>, Error> {
		loop {
			let notify = {
				let state = self.state.lock();
				if self.index < state.segments.len() {
					let segment = state.segments[self.index].clone();
					self.index += 1;
					return Ok(Some(segment));
				}

				match state.closed {
					Err(Error::Closed) => return Ok(None),
					Err(err) => return Err(err),
					Ok(()) => state.changed(),
				}
			};

			notify.await
		}
	}
}

impl Deref for Subscriber {
	type Target = Info;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

// Closes the track on Drop.
#[derive(Debug)]
struct Dropped {
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
