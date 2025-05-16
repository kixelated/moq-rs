use std::string::FromUtf8Error;
use thiserror::Error;

pub trait Decode: Sized {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError>;
}

/// A decode error.
#[derive(Error, Debug, Clone)]
pub enum DecodeError {
	#[error("short buffer")]
	Short,

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
			false => Err(DecodeError::Short),
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

impl<T: Decode> Decode for Vec<T> {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
		let size = usize::decode(buf)?;

		// Don't allocate more than 1024 elements upfront
		let mut v = Vec::with_capacity(size.min(1024));

		for _ in 0..size {
			v.push(T::decode(buf)?);
		}

		Ok(v)
	}
}

impl Decode for std::time::Duration {
	fn decode<B: bytes::Buf>(buf: &mut B) -> Result<Self, DecodeError> {
		let ms = u64::decode(buf)?;
		Ok(std::time::Duration::from_micros(ms))
	}
}

impl Decode for i8 {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		if !r.has_remaining() {
			return Err(DecodeError::Short);
		}

		// This is not the usual way of encoding negative numbers.
		// i8 doesn't exist in the draft, but we use it instead of u8 for priority.
		// A default of 0 is more ergonomic for the user than a default of 128.
		Ok(((r.get_u8() as i16) - 128) as i8)
	}
}

impl Decode for bytes::Bytes {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let len = usize::decode(r)?;
		if r.remaining() < len {
			return Err(DecodeError::Short);
		}
		let bytes = r.copy_to_bytes(len);
		Ok(bytes)
	}
}
