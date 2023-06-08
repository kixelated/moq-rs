use super::Producer;
use std::time;

#[derive(Default)]
pub struct Broadcast {
	pub tracks: Producer<Track>,
}

impl Broadcast {
	pub fn new() -> Self {
		Default::default()
	}
}

#[derive(Default)]
pub struct Track {
	// The track ID as stored in the MP4
	pub id: u32,

	// Automatically remove segments after this time.
	pub ttl: Option<time::Duration>,

	// A list of segments, which are independently decodable.
	pub segments: Producer<Segment>,
}

impl Track {
	pub fn new(id: u32, ttl: Option<time::Duration>) -> Self {
		Self {
			id,
			ttl,
			segments: Producer::default(),
		}
	}
}

pub struct Segment {
	// The timestamp of the segment.
	pub timestamp: time::Duration,

	// A list of fragments that make up the segment.
	pub fragments: Producer<Fragment>,
}

impl Segment {
	pub fn new(timestamp: time::Duration) -> Self {
		Self {
			timestamp,
			fragments: Producer::default(),
		}
	}
}

pub type Fragment = Vec<u8>;
