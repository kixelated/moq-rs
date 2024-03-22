use std::{io, sync};

use super::BoundsExceeded;

pub trait Encode: Sized {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError>;
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

	#[error("i/o error: {0}")]
	Io(sync::Arc<io::Error>),
}

impl From<io::Error> for EncodeError {
	fn from(err: io::Error) -> Self {
		Self::Io(sync::Arc::new(err))
	}
}
