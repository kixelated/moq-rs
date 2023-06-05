use super::{track, Shared, Track};

use std::collections::HashMap;

#[derive(Default)]
pub struct Broadcast {
	// The list of subscribable tracks
	// NOTE: The track ID is a u32 (for now) because it matches MP4.
	pub tracks: HashMap<u32, Shared<Track>>,

	// The epoch, which increases on every update.
	pub epoch: u64,
}

impl Broadcast {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn create_track(&mut self, id: u32) -> Shared<Track> {
		self.insert_track(id, Track::new())
	}

	pub fn insert_track(&mut self, id: u32, track: Track) -> Shared<Track> {
		let owned = Shared::new(track);
		let existing = self.tracks.insert(id, owned.clone());
		assert!(existing.is_none(), "track already exists"); // TODO return a Result
		self.epoch += 1;
		owned
	}

	pub fn get_track(&mut self, id: u32) -> Option<Shared<Track>> {
		self.tracks.get_mut(&id).cloned()
	}
}

pub struct Subscriber {
	// The broadcast state
	state: Shared<Broadcast>,
}

impl Subscriber {
	pub fn new(shared: Shared<Broadcast>) -> Self {
		Self { state: shared }
	}

	// TODO support updates
	pub fn tracks(&mut self) -> HashMap<u32, track::Subscriber> {
		let state = self.state.lock();

		// Convert the broadcast::Subscriber object into multiple track::Subscriber objects.
		let tracks = &state.tracks;
		tracks
			.iter()
			.map(|(id, shared)| (*id, track::Subscriber::new(shared.clone())))
			.collect()
	}
}
