use std::{cmp, string::FromUtf8Error, time};
use thiserror::Error;

pub trait Decode: Sized {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError>;

	// Helper function to require a additional number of bytes and then decode.
	fn decode_more<B: bytes::Buf>(buf: &mut B, remain: usize) -> Result<Self, DecodeError> {
		Self::decode_cap(buf, remain)?;
		Self::decode(buf)
	}

	// Helper function to return an error if the buffer does not have enough data
	fn decode_cap<B: bytes::Buf>(buf: &mut B, remain: usize) -> Result<(), DecodeError> {
		let needed = remain.saturating_sub(buf.remaining());
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

impl Decode for u8 {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		match r.has_remaining() {
			true => Ok(r.get_u8()),
			false => Err(DecodeError::More(1)),
		}
	}
}

impl Decode for String {
	/// Decode a string with a varint length prefix.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let v = Vec::<u8>::decode(r)?;
		let str = String::from_utf8(v)?;

		Ok(str)
	}
}

impl Decode for Vec<u8> {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
		let size = usize::decode(buf)?;
		Self::decode_cap(buf, size)?;

		// Don't allocate the entire requested size to avoid a possible attack
		// Instead, we allocate up to 1024 and keep appending as we read further.
		let mut v = vec![0; cmp::min(1024, size)];
		buf.copy_to_slice(&mut v);

		Ok(v)
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
