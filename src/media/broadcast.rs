use super::track;

use anyhow::Context;
use tokio::sync::watch;

#[derive(Default)]
struct State {
	pub tracks: Vec<track::Subscriber>,
}

pub struct Publisher {
	state: watch::Sender<State>,
}

#[derive(Clone)]
pub struct Subscriber {
	state: watch::Receiver<State>,
	index: usize,
}

impl State {
	fn new() -> Self {
		Default::default()
	}
}

impl Publisher {
	pub fn new() -> Self {
		let (state, _) = watch::channel(State::new());
		Self { state }
	}

	pub fn add_track(&mut self, track: &track::Publisher) {
		self.state.send_modify(|broadcast| {
			broadcast.tracks.push(track.subscribe());
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

	pub async fn next_track(&mut self) -> anyhow::Result<track::Subscriber> {
		let broadcast = self
			.state
			.wait_for(|broadcast| self.index < broadcast.tracks.len())
			.await
			.context("publisher dropped without close")?;

		let track = broadcast.tracks[self.index].clone();

		self.index += 1;
		Ok(track)
	}
}
