use super::segment;

use std::collections::VecDeque;

use anyhow::Context;
use tokio::sync::watch;

#[derive(Default)]
struct State {
	// The track ID as stored in the MP4
	id: u32,

	// A list of segments, which are independently decodable.
	segments: VecDeque<segment::Subscriber>,

	// If the track has finished producing segments.
	fin: bool,

	// The number of segments removed from the front of the queue.
	// ID = pruned + index
	pruned: usize,
}

pub struct Publisher {
	// Sends state updates; this is the only copy.
	state: watch::Sender<State>,
}

#[derive(Clone)]
pub struct Subscriber {
	state: watch::Receiver<State>,
	index: usize,
}

impl State {
	pub fn new(id: u32, capacity: usize) -> Self {
		Self {
			id,
			segments: VecDeque::with_capacity(capacity),
			pruned: 0,
			fin: false,
		}
	}
}

impl Publisher {
	// TODO represent capacity in units of time (or bytes?)
	pub fn new(id: u32, capacity: usize) -> Self {
		let init = State::new(id, capacity);
		let (state, _) = watch::channel(init);
		Self { state }
	}

	pub fn push_segment(&mut self, segment: &segment::Publisher) {
		self.state.send_modify(|state| {
			// Remove segments from the front as we will up on capacity.
			if state.segments.capacity() == state.segments.len() {
				state.segments.pop_front();
				state.pruned += 1;
			}

			state.segments.push_back(segment.subscribe());
		});
	}

	pub fn close(&mut self) {
		self.state.send_modify(|state| state.fin = true);
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber::new(self.state.subscribe())
	}
}

impl Subscriber {
	fn new(state: watch::Receiver<State>) -> Self {
		Self { state, index: 0 }
	}

	pub fn id(&self) -> u32 {
		self.state.borrow().id
	}

	pub async fn next_segment(&mut self) -> anyhow::Result<Option<segment::Subscriber>> {
		let state = self
			.state
			.wait_for(|state| state.fin || self.index < state.pruned + state.segments.len())
			.await
			.context("publisher dropped without close")?;

		let index = self.index.saturating_sub(state.pruned);
		if index < state.segments.len() {
			let segment = state.segments[index].clone();
			self.index = index + state.pruned + 1;
			Ok(Some(segment))
		} else if state.fin {
			Ok(None)
		} else {
			panic!("impossible state")
		}
	}
}
