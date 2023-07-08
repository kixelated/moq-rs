use super::VarInt;
use bytes::{Buf, Bytes};
use std::str;

use thiserror::Error;

#[derive(Error, Debug)]
pub enum DecodeError {
	#[error("unexpected end of buffer")]
	UnexpectedEnd,

	#[error("invalid string")]
	InvalidString(#[from] str::Utf8Error),

	#[error("invalid type: {0:?}")]
	InvalidType(VarInt),

	#[error("unknown error")]
	Unknown,
}

pub trait Decode: Sized {
	// Decodes a message, returning UnexpectedEnd if there's not enough bytes in the buffer.
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError>;
}

impl Decode for Bytes {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let size = VarInt::decode(r)?.into_inner() as usize;
		if r.remaining() < size {
			return Err(DecodeError::UnexpectedEnd);
		}

		let buf = r.copy_to_bytes(size);
		Ok(buf)
	}
}

impl Decode for Vec<u8> {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Bytes::decode(r).map(|b| b.to_vec())
	}
}

impl Decode for String {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let data = Vec::decode(r)?;
		let s = str::from_utf8(&data)?.to_string();
		Ok(s)
	}
}

impl Decode for u8 {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		if r.remaining() < 1 {
			return Err(DecodeError::UnexpectedEnd);
		}

		Ok(r.get_u8())
	}
}

/*
impl<const N: usize> Decode for [u8; N] {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		if r.remaining() < N {
			return Err(DecodeError::UnexpectedEnd);
		}

		let mut buf = [0; N];
		r.copy_to_slice(&mut buf);

		Ok(buf)
	}
}
*/
