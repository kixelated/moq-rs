use crate::coding;

#[derive(Debug, thiserror::Error)]
pub enum Error {
	#[error("decode error: {0}")]
	Coding(#[from] coding::DecodeError),
}
