use super::{BoundsExceeded, VarInt};
use bytes::{BufMut, Bytes};

use thiserror::Error;

#[derive(Error, Debug)]
pub enum EncodeError {
	#[error("unexpected end of buffer")]
	UnexpectedEnd,

	#[error("varint too large")]
	BoundsExceeded(#[from] BoundsExceeded),

	#[error("unknown error")]
	Unknown,
}

pub trait Encode: Sized {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError>;
}

impl Encode for Bytes {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.as_ref().encode(w)
	}
}

impl Encode for Vec<u8> {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.as_slice().encode(w)
	}
}

impl Encode for &[u8] {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let size = VarInt::try_from(self.len())?;
		size.encode(w)?;

		if w.remaining_mut() < self.len() {
			return Err(EncodeError::UnexpectedEnd);
		}
		w.put_slice(self);

		Ok(())
	}
}

impl Encode for String {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.as_bytes().encode(w)
	}
}

impl Encode for u8 {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		if w.remaining_mut() < 1 {
			return Err(EncodeError::UnexpectedEnd);
		}

		w.put_u8(*self);
		Ok(())
	}
}

impl Encode for u16 {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		if w.remaining_mut() < 2 {
			return Err(EncodeError::UnexpectedEnd);
		}

		w.put_u16(*self);
		Ok(())
	}
}

impl Encode for u32 {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		if w.remaining_mut() < 4 {
			return Err(EncodeError::UnexpectedEnd);
		}

		w.put_u32(*self);
		Ok(())
	}
}
impl Encode for u64 {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		if w.remaining_mut() < 8 {
			return Err(EncodeError::UnexpectedEnd);
		}

		w.put_u64(*self);
		Ok(())
	}
}
