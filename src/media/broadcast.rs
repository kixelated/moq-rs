use super::track;

use tokio::sync::watch;

#[derive(Default)]
pub struct Broadcast {
	tracks: Vec<track::Consumer>,

	closed: bool,
}

pub struct Producer {
	state: watch::Sender<Broadcast>,
}

#[derive(Clone)]
pub struct Consumer {
	state: watch::Receiver<Broadcast>,
	index: usize,
}

impl Broadcast {
	pub fn new() -> Producer {
		let broadcast = Default::default();
		Producer::new(broadcast)
	}

	pub fn add_track(&mut self, track: track::Consumer) {
		self.tracks.push(track);
	}
}

impl Producer {
	pub fn new(broadcast: Broadcast) -> Self {
		let (state, _) = watch::channel(broadcast);
		Self { state }
	}

	pub fn add_track(&mut self, track: track::Consumer) {
		self.state.send_modify(|state| {
			state.add_track(track);
		});
	}

	pub fn subscribe(&self) -> Consumer {
		Consumer::new(self.state.subscribe())
	}
}

impl Drop for Producer {
	fn drop(&mut self) {
		self.state.send_modify(|state| state.closed = true);
	}
}

impl Consumer {
	fn new(state: watch::Receiver<Broadcast>) -> Self {
		Self { state, index: 0 }
	}

	pub async fn next_track(&mut self) -> Option<track::Consumer> {
		let broadcast = self
			.state
			.wait_for(|state| state.closed || self.index < state.tracks.len())
			.await
			.expect("publisher dropped without close");

		if self.index < broadcast.tracks.len() {
			let track = broadcast.tracks[self.index].clone();
			self.index += 1;

			Some(track)
		} else {
			None
		}
	}

	pub fn lock(&self) -> watch::Ref<Broadcast> {
		self.state.borrow()
	}
}
