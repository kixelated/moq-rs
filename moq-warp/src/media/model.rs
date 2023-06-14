use super::Subscriber;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Duration;

// Map from track namespace to broadcast.
// TODO support updates
pub type Broadcasts = Arc<HashMap<String, Broadcast>>;

#[derive(Clone)]
pub struct Broadcast {
	// TODO support updates.
	pub tracks: Arc<HashMap<String, Track>>,
}

#[derive(Clone)]
pub struct Track {
	// A list of segments, which are independently decodable.
	pub segments: Subscriber<Segment>,
}

#[derive(Clone)]
pub struct Segment {
	// The timestamp of the segment.
	pub timestamp: Duration,

	// A list of fragments that make up the segment.
	pub fragments: Subscriber<Fragment>,
}

// Use Arc to avoid cloning the entire MP4 data for each subscriber.
pub type Fragment = Arc<Vec<u8>>;
