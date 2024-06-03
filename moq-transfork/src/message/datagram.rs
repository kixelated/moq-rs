use crate::coding::*;

pub enum Datagram {
	Group,
	Frame,
}

impl Decode for Datagram {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let t = u64::decode(r)?;
		match t {
			0 => Ok(Self::Group),
			1 => Ok(Self::Frame),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for Datagram {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::Group => 0u64,
			Self::Frame => 1u64,
		}
		.encode(w)
	}
}
