use super::BoundsExceeded;
use std::{io, string::FromUtf8Error, sync};
use thiserror::Error;

pub trait Decode: Sized {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError>;

	// Helper function to make sure we have enough bytes to decode
	fn decode_remaining<B: bytes::Buf>(buf: &mut B, required: usize) -> Result<(), DecodeError> {
		let needed = required.saturating_sub(buf.remaining());
		if needed > 0 {
			Err(DecodeError::More(needed))
		} else {
			Ok(())
		}
	}
}

/// A decode error.
#[derive(Error, Debug, Clone)]
pub enum DecodeError {
	#[error("fill buffer")]
	More(usize),

	#[error("invalid string")]
	InvalidString(#[from] FromUtf8Error),

	#[error("invalid message: {0:?}")]
	InvalidMessage(u64),

	#[error("invalid role: {0:?}")]
	InvalidRole(u64),

	#[error("invalid subscribe location")]
	InvalidSubscribeLocation,

	#[error("invalid filter type")]
	InvalidFilterType,

	#[error("invalid object status")]
	InvalidObjectStatus,

	#[error("invalid track status code")]
	InvalidTrackStatusCode,

	#[error("missing field")]
	MissingField,

	#[error("invalid value")]
	InvalidValue,

	#[error("varint bounds exceeded")]
	BoundsExceeded(#[from] BoundsExceeded),

	// TODO move these to ParamError
	#[error("duplicate parameter")]
	DupliateParameter,

	#[error("missing parameter")]
	MissingParameter,

	#[error("invalid parameter")]
	InvalidParameter,

	#[error("io error: {0}")]
	Io(sync::Arc<io::Error>),
}

impl From<io::Error> for DecodeError {
	fn from(err: io::Error) -> Self {
		Self::Io(sync::Arc::new(err))
	}
}
