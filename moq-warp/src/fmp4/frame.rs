use bytes::Bytes;

use super::Timestamp;

#[derive(Debug, Clone)]
pub struct Frame {
	pub timestamp: Timestamp,
	pub keyframe: bool,
	pub payload: Bytes,

	pub raw: Bytes,
}
