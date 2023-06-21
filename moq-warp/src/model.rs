use super::Subscriber;
use std::sync::Arc;
use std::time::Instant;

use moq_transport::VarInt;

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
