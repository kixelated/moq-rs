use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Filter Types
/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html#name-filter-types
#[derive(Clone, Debug, PartialEq)]
pub enum FilterType {
	LatestGroup = 0x1,
	LatestObject = 0x2,
	AbsoluteStart = 0x3,
	AbsoluteRange = 0x4,
}

impl Encode for FilterType {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::LatestGroup => (0x1_u64).encode(w),
			Self::LatestObject => (0x2_u64).encode(w),
			Self::AbsoluteStart => (0x3_u64).encode(w),
			Self::AbsoluteRange => (0x4_u64).encode(w),
		}
	}
}

impl Decode for FilterType {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0x01 => Ok(Self::LatestGroup),
			0x02 => Ok(Self::LatestObject),
			0x03 => Ok(Self::AbsoluteStart),
			0x04 => Ok(Self::AbsoluteRange),
			_ => Err(DecodeError::InvalidFilterType),
		}
	}
}
