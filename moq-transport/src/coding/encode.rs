use std::{io, sync};

use super::BoundsExceeded;

pub trait Encode: Sized {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError>;

	// Helper function to make sure we have enough bytes to encode
	fn encode_remaining<W: bytes::BufMut>(buf: &mut W, required: usize) -> Result<(), EncodeError> {
		let needed = required.saturating_sub(buf.remaining_mut());
		if needed > 0 {
			Err(EncodeError::More(needed))
		} else {
			Ok(())
		}
	}
}

/// An encode error.
#[derive(thiserror::Error, Debug, Clone)]
pub enum EncodeError {
	#[error("short buffer")]
	More(usize),

	#[error("varint too large")]
	BoundsExceeded(#[from] BoundsExceeded),

	#[error("invalid value")]
	InvalidValue,

	#[error("missing field")]
	MissingField,

	#[error("i/o error: {0}")]
	Io(sync::Arc<io::Error>),
}

impl From<io::Error> for EncodeError {
	fn from(err: io::Error) -> Self {
		Self::Io(sync::Arc::new(err))
	}
}
