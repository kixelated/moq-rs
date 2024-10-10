use crate::coding::*;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum Stream {
	Session,
	Announce,
	Subscribe,
	Fetch,
	Info,
}

impl Decode for Stream {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let t = u64::decode(r)?;
		match t {
			0 => Ok(Self::Session),
			1 => Ok(Self::Announce),
			2 => Ok(Self::Subscribe),
			3 => Ok(Self::Fetch),
			4 => Ok(Self::Info),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for Stream {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		let v: u64 = match self {
			Self::Session => 0,
			Self::Announce => 1,
			Self::Subscribe => 2,
			Self::Fetch => 3,
			Self::Info => 4,
		};
		v.encode(w)
	}
}

#[derive(Debug, PartialEq, Clone, Copy)]
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
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		let v: u64 = match self {
			Self::Group => 0,
		};
		v.encode(w)
	}
}
