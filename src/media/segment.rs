use super::{Fragment, Shared};

use std::sync::Arc;

#[derive(Default)]
pub struct Segment {
	// A list of fragments that make up the segment.
	pub fragments: Vec<Arc<Fragment>>,

	// Whether the final fragment has been pushed.
	pub fin: bool,
}

impl Segment {
	pub fn new() -> Self {
		Self::default()
	}

	pub fn push_fragment(&mut self, data: Fragment) {
		let owned = Arc::new(data);
		self.fragments.push(owned);
	}
}

pub struct Subscriber {
	// The segment
	state: Shared<Segment>,

	// The last seen index.
	index: usize,
}

impl Subscriber {
	pub fn new(state: Shared<Segment>) -> Self {
		Self { state, index: 0 }
	}

	// TODO support futures
	pub fn fragment(&mut self) -> Option<Arc<Fragment>> {
		let state = self.state.lock();

		if self.index >= state.fragments.len() {
			let fragment = state.fragments[self.index].clone();
			self.index += 1;

			Some(fragment)
		} else {
			None
		}
	}

	pub fn done(&mut self) -> bool {
		// TODO avoid needing a mutable lock
		let state = self.state.lock();
		state.fin && self.index >= state.fragments.len()
	}
}
