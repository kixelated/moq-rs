use std::fmt;

use moq_transfork_proto::coding::*;

use derive_more::Debug;

use crate::Timestamp;

#[derive(Clone, Debug)]
pub struct Frame {
	pub timestamp: Timestamp,
	pub keyframe: bool,

	#[debug("{}", payload.len())]
	pub payload: Bytes,
}
