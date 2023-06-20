use super::Subscriber;
use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use moq_transport::VarInt;

#[derive(Clone, Default)]
pub struct Broadcast {
	// TODO support updates.
	pub tracks: Arc<HashMap<String, Track>>,
}

impl fmt::Debug for Broadcast {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Broadcast")
			.field("tracks", &self.tracks.keys())
			.finish()
	}
}

#[derive(Clone)]
pub struct Track {
	// A list of segments, which are independently decodable.
	pub segments: Subscriber<Segment>,
}

#[derive(Clone)]
pub struct Segment {
	// The sequence number of the segment within the track.
	pub sequence: VarInt,

	// The priority of the segment within the BROADCAST.
	pub send_order: VarInt,

	// The time at which the segment expires for cache purposes.
	pub expires: Option<Instant>,

	// A list of fragments that make up the segment.
	pub fragments: Subscriber<Fragment>,
}

// Use Arc to avoid cloning the entire MP4 data for each subscriber.
pub type Fragment = Arc<Vec<u8>>;
