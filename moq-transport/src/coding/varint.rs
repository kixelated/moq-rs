// Based on quinn-proto
// https://github.com/quinn-rs/quinn/blob/main/quinn-proto/src/varint.rs
// Licensed via Apache 2.0 and MIT

use std::convert::{TryFrom, TryInto};
use std::fmt;

use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};
use thiserror::Error;

#[derive(Debug, Copy, Clone, Eq, PartialEq, Error)]
#[error("value too large for varint encoding")]
pub struct BoundsExceeded;

/// An integer less than 2^62
///
/// Values of this type are suitable for encoding as QUIC variable-length integer.
// It would be neat if we could express to Rust that the top two bits are available for use as enum
// discriminants
#[derive(Default, Copy, Clone, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct VarInt(u64);

impl VarInt {
	pub const MAX: Self = Self((1 << 62) - 1);

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
		if x <= Self::MAX.into_inner() {
			Ok(Self(x))
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
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let mut buf = [0; 8];

		if r.remaining() < 1 {
			return Err(DecodeError::UnexpectedEnd);
		}

		buf[0] = r.get_u8();

		let tag = buf[0] >> 6;
		buf[0] &= 0b0011_1111;

		let x = match tag {
			0b00 => u64::from(buf[0]),
			0b01 => {
				if r.remaining() < 1 {
					return Err(DecodeError::UnexpectedEnd);
				}

				r.copy_to_slice(buf[1..2].as_mut());
				u64::from(u16::from_be_bytes(buf[..2].try_into().unwrap()))
			}
			0b10 => {
				if r.remaining() < 3 {
					return Err(DecodeError::UnexpectedEnd);
				}

				r.copy_to_slice(buf[1..4].as_mut());
				u64::from(u32::from_be_bytes(buf[..4].try_into().unwrap()))
			}
			0b11 => {
				if r.remaining() < 7 {
					return Err(DecodeError::UnexpectedEnd);
				}

				r.copy_to_slice(buf[1..8].as_mut());
				u64::from_be_bytes(buf)
			}
			_ => unreachable!(),
		};

		Ok(Self(x))
	}
}

impl Encode for VarInt {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let x = self.0;
		if x < 2u64.pow(6) {
			(x as u8).encode(w)
		} else if x < 2u64.pow(14) {
			(0b01 << 14 | x as u16).encode(w)
		} else if x < 2u64.pow(30) {
			(0b10 << 30 | x as u32).encode(w)
		} else if x < 2u64.pow(62) {
			(0b11 << 62 | x).encode(w)
		} else {
			unreachable!("malformed VarInt");
		}
	}
}
