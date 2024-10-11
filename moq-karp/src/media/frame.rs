use std::fmt;

use moq_transfork::coding::*;

use super::Timestamp;

#[derive(Clone)]
pub struct Frame {
	pub timestamp: Timestamp,
	pub keyframe: bool,
	pub payload: Bytes,
}

impl fmt::Debug for Frame {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		f.debug_struct("Frame")
			.field("timestamp", &self.timestamp)
			.field("keyframe", &self.keyframe)
			.field("payload_len", &self.payload.len())
			.finish()
	}
}
