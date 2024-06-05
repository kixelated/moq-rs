use crate::coding::*;

#[derive(Debug, PartialEq, Clone)]
pub enum StreamUni {
	Group,
}

impl Decode for StreamUni {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let t = u64::decode(r)?;
		match t {
			0 => Ok(Self::Group),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for StreamUni {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = match self {
			Self::Group => 0,
		};
		v.encode(w)
	}
}
