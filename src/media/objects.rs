use std::collections::{HashMap, VecDeque};

use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default)]
pub struct Broadcast {
	// The list of subscribable tracks
	// NOTE: The track ID is a u32 (for now) because it matches MP4.
	tracks: HashMap<u32, Arc<Track>>,

	// The epoch, which increases on every update.
	epoch: u64,
}

impl Broadcast {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn add_track(&mut self, id: u32, track: Track) -> Arc<Track> {
		let owned = Arc::new(track);
		self.tracks.insert(id, owned);
		self.epoch += 1;

		owned
	}

	pub fn get_track(&self, id: u32) -> Option<Arc<Track>> {
		self.tracks.get(&id).cloned()
	}
}

pub struct Track {
	// The number of segments removed from the front of the queue.
	// ID = offset + index
	offset: usize,

	// A list of segments, which are independently decodable.
	segments: VecDeque<Arc<Segment>>,

	// The epoch, which updates on every update.
	pub epoch: u64,
}

impl Track {
	pub fn new() -> Self {
		Self {
			offset: 0,
			segments: VecDeque::new(),
			epoch: 0,
		}
	}

	pub fn add_segment(&mut self, segment: Segment) -> Arc<Segment> {
		let owned = Arc::new(segment);
		self.segments.push_back(owned);
		self.epoch += 1;
		owned
	}

	pub fn get_segment(&self, index: usize) -> Option<Arc<Segment>> {
		self.segments.get(id - self.offset).cloned()
	}

	pub fn last_segment(&self) -> Option<Arc<Segment>> {
		self.segments.back().cloned()
	}

	// TODO based on timestamp, not size
	pub fn expire_segments(&mut self, max: usize) {
		while (self.segments.len() > max) {
			self.segments.pop_front();
			self.offset += 1;
		}
	}
}

#[derive(Default)]
pub struct Segment {
	// A list of fragments that make up the segment.
	fragments: Vec<Fragment>,

	// The epoch, which updates on every update.
	pub epoch: u64,
}

impl Segment {
	pub fn new() -> Self {
		Default::default()
	}

	pub fn add_fragment(&mut self, data: Fragment) {
		self.fragments.push(data);
		self.epoch += 1;
	}

	pub fn get_fragment(&self, id: usize) -> Option<Fragment> {
		self.fragments.get(id).cloned()
	}
}

pub type Fragment = Vec<u8>;
