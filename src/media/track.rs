use super::{Segment, Shared};

use std::collections::VecDeque;

#[derive(Default)]
pub struct Track {
	// The number of segments removed from the front of the queue.
	// ID = offset + index
	pub offset: usize,

	// A list of segments, which are independently decodable.
	pub segments: VecDeque<Shared<Segment>>,

	// If the track has finished producing segments.
	pub fin: bool,
}

impl Track {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn create_segment(&mut self) -> Shared<Segment> {
		self.push_segment(Segment::new())
	}

	pub fn push_segment(&mut self, segment: Segment) -> Shared<Segment> {
		let owned = Shared::new(segment);
		self.segments.push_back(owned.clone());
		owned
	}

	// TODO based on timestamp, not size
	pub fn expire(&mut self, max: usize) {
		while self.segments.len() > max {
			self.segments.pop_front();
			self.offset += 1;
		}
	}

	pub fn done(&mut self) {
		self.fin = true;
	}
}

pub enum Error {
	Wait,
	Final,
}

pub struct Subscriber {
	// The track state
	state: Shared<Track>,

	// The last seen index.
	index: usize,
}

impl Subscriber {
	pub fn new(state: Shared<Track>) -> Self {
		Self { state, index: 0 }
	}

	// TODO support futures
	pub fn segment(&mut self) -> Option<Shared<Segment>> {
		let state = self.state.lock();

		if self.index < state.offset + state.segments.len() {
			let segment = state.segments[self.index - state.offset].clone();
			self.index += 1;

			Some(segment)
		} else {
			None
		}
	}

	pub fn done(&mut self) -> bool {
		// TODO avoid a mutable lock
		let state = self.state.lock();
		state.fin && self.index >= state.offset + state.segments.len()
	}
}
