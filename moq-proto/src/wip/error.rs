use derive_more::{From, Into};

use crate::coding;

#[derive(Clone, Debug, thiserror::Error)]
pub enum Error {
	#[error("decode error: {0}")]
	Coding(#[from] coding::DecodeError),

	#[error("stream closed: {0}")]
	Closed(u8),

	#[error("unknown stream")]
	UnknownStream,

	#[error("duplicate stream")]
	DuplicateStream,

	#[error("wrong stream type")]
	WrongStreamType,

	#[error("poisoned")]
	Poisoned,
}

#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone, From, Into, PartialOrd, Ord)]
pub struct ErrorCode(u32);
