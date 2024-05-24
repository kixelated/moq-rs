// Based on quinn-proto
// https://github.com/quinn-rs/quinn/blob/main/quinn-proto/src/varint.rs
// Licensed via Apache 2.0 and MIT

use std::convert::{TryFrom, TryInto};
use std::fmt;

use thiserror::Error;

use super::{Decode, DecodeError, Encode, EncodeError};

#[derive(Debug, Copy, Clone, Eq, PartialEq, Error)]
#[error("value out of range")]
pub struct BoundsExceeded;

/// An integer less than 2^62
///
/// Values of this type are suitable for encoding as QUIC variable-length integer.
/// It would be neat if we could express to Rust that the top two bits are available for use as enum
/// discriminants
#[derive(Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VarInt(u64);

impl VarInt {
	/// The largest possible value.
	pub const MAX: Self = Self((1 << 62) - 1);

	/// The smallest possible value.
	pub const ZERO: Self = Self(0);

	/// Construct a `VarInt` infallibly using the largest available type.
	/// Larger values need to use `try_from` instead.
	pub const fn from_u32(x: u32) -> Self {
		Self(x as u64)
	}

	/// Extract the integer value
	pub const fn into_inner(self) -> u64 {
		self.0
	}
}

impl From<VarInt> for u64 {
	fn from(x: VarInt) -> Self {
		x.0
	}
}

impl From<VarInt> for usize {
	fn from(x: VarInt) -> Self {
		x.0 as usize
	}
}

impl From<VarInt> for u128 {
	fn from(x: VarInt) -> Self {
		x.0 as u128
	}
}

impl From<u8> for VarInt {
	fn from(x: u8) -> Self {
		Self(x.into())
	}
}

impl From<u16> for VarInt {
	fn from(x: u16) -> Self {
		Self(x.into())
	}
}

impl From<u32> for VarInt {
	fn from(x: u32) -> Self {
		Self(x.into())
	}
}

impl TryFrom<u64> for VarInt {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^62
	fn try_from(x: u64) -> Result<Self, BoundsExceeded> {
		let x = Self(x);
		if x <= Self::MAX {
			Ok(x)
		} else {
			Err(BoundsExceeded)
		}
	}
}

impl TryFrom<u128> for VarInt {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^62
	fn try_from(x: u128) -> Result<Self, BoundsExceeded> {
		if x <= Self::MAX.into() {
			Ok(Self(x as u64))
		} else {
			Err(BoundsExceeded)
		}
	}
}

impl TryFrom<usize> for VarInt {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^62
	fn try_from(x: usize) -> Result<Self, BoundsExceeded> {
		Self::try_from(x as u64)
	}
}

impl TryFrom<VarInt> for u32 {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^32
	fn try_from(x: VarInt) -> Result<Self, BoundsExceeded> {
		if x.0 <= u32::MAX.into() {
			Ok(x.0 as u32)
		} else {
			Err(BoundsExceeded)
		}
	}
}

impl TryFrom<VarInt> for u16 {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^16
	fn try_from(x: VarInt) -> Result<Self, BoundsExceeded> {
		if x.0 <= u16::MAX.into() {
			Ok(x.0 as u16)
		} else {
			Err(BoundsExceeded)
		}
	}
}

impl TryFrom<VarInt> for u8 {
	type Error = BoundsExceeded;

	/// Succeeds iff `x` < 2^8
	fn try_from(x: VarInt) -> Result<Self, BoundsExceeded> {
		if x.0 <= u8::MAX.into() {
			Ok(x.0 as u8)
		} else {
			Err(BoundsExceeded)
		}
	}
}

impl fmt::Debug for VarInt {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl fmt::Display for VarInt {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		self.0.fmt(f)
	}
}

impl Decode for VarInt {
	/// Decode a varint from the given reader.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Self::decode_remaining(r, 1)?;

		let b = r.get_u8();
		let tag = b >> 6;

		let mut buf = [0u8; 8];
		buf[0] = b & 0b0011_1111;

		let x = match tag {
			0b00 => u64::from(buf[0]),
			0b01 => {
				Self::decode_remaining(r, 1)?;
				r.copy_to_slice(buf[1..2].as_mut());
				u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap()))
			}
			0b10 => {
				Self::decode_remaining(r, 3)?;
				r.copy_to_slice(buf[1..4].as_mut());
				u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap()))
			}
			0b11 => {
				Self::decode_remaining(r, 7)?;
				r.copy_to_slice(buf[1..8].as_mut());
				u64::from_be_bytes(buf)
			}
			_ => unreachable!(),
		};

		Ok(Self(x))
	}
}

impl Encode for VarInt {
	/// Encode a varint to the given writer.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let x = self.0;
		if x < 2u64.pow(6) {
			Self::encode_remaining(w, 1)?;
			w.put_u8(x as u8)
		} else if x < 2u64.pow(14) {
			Self::encode_remaining(w, 2)?;
			w.put_u16(0b01 << 14 | x as u16)
		} else if x < 2u64.pow(30) {
			Self::encode_remaining(w, 4)?;
			w.put_u32(0b10 << 30 | x as u32)
		} else if x < 2u64.pow(62) {
			Self::encode_remaining(w, 8)?;
			w.put_u64(0b11 << 62 | x)
		} else {
			return Err(BoundsExceeded.into());
		}

		Ok(())
	}
}

impl Encode for u64 {
	/// Encode a varint to the given writer.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let var = VarInt::try_from(*self)?;
		var.encode(w)
	}
}

impl Decode for u64 {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		VarInt::decode(r).map(|v| v.into_inner())
	}
}

impl Encode for usize {
	/// Encode a varint to the given writer.
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let var = VarInt::try_from(*self)?;
		var.encode(w)
	}
}

impl Decode for usize {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		VarInt::decode(r).map(|v| v.into_inner() as usize)
	}
}
