use moq_transfork::coding::*;

use derive_more::Debug;

pub type Timestamp = std::time::Duration;

#[derive(Clone, Debug)]
pub struct Frame {
	pub timestamp: Timestamp,
	pub keyframe: bool,

	#[debug("{}", payload.len())]
	pub payload: Bytes,
}
