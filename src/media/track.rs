use super::segment;

use std::collections::VecDeque;
use std::time;

use tokio::sync::watch;

// TODO split into static and dynamic components
#[derive(Default)]
pub struct Track {
	// The track ID as stored in the MP4
	pub id: u32,

	// Automatically remove segments after this time.
	pub ttl: Option<time::Duration>,

	// A list of segments, which are independently decodable.
	segments: VecDeque<segment::Consumer>,

	// The number of segments removed from the front of the queue.
	// ID = pruned + index
	pruned: usize,

	// If the track has finished updates.
	closed: bool,
}

impl Track {
	pub fn new(id: u32, ttl: Option<time::Duration>) -> Producer {
		let track = Self {
			id,
			ttl,
			segments: VecDeque::new(),
			pruned: 0,
			closed: false,
		};

		Producer::new(track)
	}

	pub fn push_segment(&mut self, segment: segment::Consumer) {
		self.segments.push_back(segment);

		if let Some(ttl) = self.ttl {
			// Compute the last allowed timestamp in the queue.
			let expires = self.segments.back().unwrap().lock().timestamp.saturating_sub(ttl);

			// Remove segments from the front based on their timestamp.
			// TODO this assumes segments are pushed in chronological order, which is not true.
			while let Some(front) = self.segments.front() {
				if front.lock().timestamp <= expires {
					break;
				}

				self.segments.pop_front();
				self.pruned += 1;
			}
		}
	}
}

pub struct Producer {
	// Sends state updates; this is the only copy.
	state: watch::Sender<Track>,
}

impl Producer {
	pub fn new(track: Track) -> Self {
		let (state, _) = watch::channel(track);
		Self { state }
	}

	pub fn push_segment(&mut self, segment: segment::Consumer) {
		self.state.send_modify(|state| state.push_segment(segment));
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

#[derive(Clone)]
pub struct Consumer {
	state: watch::Receiver<Track>,
	index: usize,
}

impl Consumer {
	fn new(state: watch::Receiver<Track>) -> Self {
		Self { state, index: 0 }
	}

	pub fn id(&self) -> u32 {
		self.state.borrow().id
	}

	pub async fn next_segment(&mut self) -> Option<segment::Consumer> {
		let state = self
			.state
			.wait_for(|state| state.closed || self.index < state.pruned + state.segments.len())
			.await
			.expect("publisher dropped without close");

		let index = self.index.saturating_sub(state.pruned);
		if index < state.segments.len() {
			let segment = state.segments[index].clone();
			self.index = index + state.pruned + 1;

			Some(segment)
		} else {
			None
		}
	}

	pub fn lock(&self) -> watch::Ref<Track> {
		self.state.borrow()
	}
}
