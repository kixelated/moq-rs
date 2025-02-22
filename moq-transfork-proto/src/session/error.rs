use crate::coding;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("decode error: {0}")]
	Coding(#[from] coding::DecodeError),

	#[error("stream closed: {0}")]
	Closed(u8),

	#[error("unknown stream")]
	UnknownStream,

	#[error("poisoned")]
	Poisoned,
}
