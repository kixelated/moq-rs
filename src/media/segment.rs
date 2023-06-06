use super::fragment;

use anyhow::Context;
use std::sync::Arc;
use tokio::sync::watch;

#[derive(Default)]
struct State {
	// A list of fragments that make up the segment.
	fragments: Vec<fragment::Shared>,

	// Whether the final fragment has been pushed.
	fin: bool,
}

impl State {
	fn new() -> Self {
		Default::default()
	}
}

pub struct Publisher {
	state: watch::Sender<State>,
}

#[derive(Clone)]
pub struct Subscriber {
	state: watch::Receiver<State>,

	// The last seen index.
	index: usize,
}

impl Publisher {
	pub fn new() -> Self {
		let init = State::new();
		let (state, _) = watch::channel(init);

		Self { state }
	}

	pub fn push_fragment(&mut self, fragment: fragment::Data) {
		let owned = Arc::new(fragment);

		self.state.send_modify(|state| {
			state.fragments.push(owned);
		});
	}

	pub fn close(&mut self) {
		self.state.send_modify(|state| {
			state.fin = true;
		});
	}

	pub fn subscribe(&self) -> Subscriber {
		Subscriber::new(self.state.subscribe())
	}
}

impl Default for Publisher {
	fn default() -> Self {
		Self::new()
	}
}

impl Subscriber {
	fn new(state: watch::Receiver<State>) -> Self {
		Self { state, index: 0 }
	}

	pub async fn next_fragment(&mut self) -> anyhow::Result<Option<fragment::Shared>> {
		let state = self
			.state
			.wait_for(|segment| segment.fin || self.index < segment.fragments.len())
			.await
			.context("publisher dropped without close")?;

		if self.index < state.fragments.len() {
			let fragment = state.fragments[self.index].clone();
			self.index += 1;

			Ok(Some(fragment))
		} else {
			Ok(None)
		}
	}
}
