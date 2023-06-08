use super::fragment;

use std::{sync, time};

use tokio::sync::watch;

pub struct Segment {
	// The timestamp of the segment.
	pub timestamp: time::Duration,

	// A list of fragments that make up the segment.
	fragments: Vec<fragment::Shared>,

	// Whether the segment is finished updated.
	closed: bool,
}

impl Segment {
	pub fn new(timestamp: time::Duration) -> Producer {
		let state = Self {
			timestamp,
			fragments: Vec::new(),
			closed: false,
		};

		Producer::new(state)
	}

	pub fn push_fragment(&mut self, fragment: fragment::Data) {
		let owned = sync::Arc::new(fragment);
		self.fragments.push(owned);
	}
}

pub struct Producer {
	state: watch::Sender<Segment>,
}

#[derive(Clone)]
pub struct Consumer {
	state: watch::Receiver<Segment>,

	// The last seen index.
	index: usize,
}

impl Producer {
	pub fn new(segment: Segment) -> Self {
		let (state, _) = watch::channel(segment);
		Self { state }
	}

	pub fn push_fragment(&mut self, fragment: fragment::Data) {
		self.state.send_modify(|state| state.push_fragment(fragment));
	}

	pub fn subscribe(&self) -> Consumer {
		Consumer::new(self.state.subscribe())
	}
}

impl Drop for Producer {
	fn drop(&mut self) {
		self.state.send_modify(|state| {
			state.closed = true;
		});
	}
}

impl Consumer {
	fn new(state: watch::Receiver<Segment>) -> Self {
		Self { state, index: 0 }
	}

	pub async fn next_fragment(&mut self) -> Option<fragment::Shared> {
		let state = self
			.state
			.wait_for(|segment| segment.closed || self.index < segment.fragments.len())
			.await
			.expect("publisher dropped without close");

		if self.index < state.fragments.len() {
			let fragment = state.fragments[self.index].clone();
			self.index += 1;

			Some(fragment)
		} else {
			None
		}
	}

	pub fn lock(&self) -> watch::Ref<Segment> {
		self.state.borrow()
	}
}
