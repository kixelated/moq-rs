use std::{string::FromUtf8Error, time};
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

	#[error("invalid value")]
	InvalidValue,

	#[error("bounds exceeded")]
	BoundsExceeded,

	#[error("expected end")]
	ExpectedEnd,

	#[error("expected data")]
	ExpectedData,

	// TODO move these to ParamError
	#[error("duplicate parameter")]
	DupliateParameter,

	#[error("missing parameter")]
	MissingParameter,

	#[error("invalid parameter")]
	InvalidParameter,
}

impl Decode for String {
	/// Decode a string with a varint length prefix.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let size = usize::decode(r)?;
		Self::decode_remaining(r, size)?;

		let mut buf = vec![0; size];
		r.copy_to_slice(&mut buf);
		let str = String::from_utf8(buf)?;

		Ok(str)
	}
}

impl Decode for Option<u64> {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(match u64::decode(r)? {
			0 => None,
			v => Some(v - 1),
		})
	}
}

impl Decode for Option<usize> {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(match usize::decode(r)? {
			0 => None,
			v => Some(v - 1),
		})
	}
}

impl Decode for time::Duration {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
		let ms = u64::decode(buf)?;
		Ok(time::Duration::from_millis(ms))
	}
}

impl Decode for Option<time::Duration> {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
		Ok(match u64::decode(buf)? {
			0 => None,
			v => Some(time::Duration::from_millis(v - 1)),
		})
	}
}
