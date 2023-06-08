use super::Subscriber;
use std::{sync, time};

#[derive(Clone)]
pub struct Broadcast {
	pub tracks: Subscriber<Track>,
}

#[derive(Clone)]
pub struct Track {
	// The track ID as stored in the MP4
	pub id: u32,

	// A list of segments, which are independently decodable.
	pub segments: Subscriber<Segment>,
}

#[derive(Clone)]
pub struct Segment {
	// The timestamp of the segment.
	pub timestamp: time::Duration,

	// A list of fragments that make up the segment.
	pub fragments: Subscriber<Fragment>,
}

// Use Arc to avoid cloning the entire MP4 data for each subscriber.
pub type Fragment = sync::Arc<Vec<u8>>;
