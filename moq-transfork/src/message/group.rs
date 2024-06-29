use crate::coding::*;

#[derive(Clone, Debug)]
pub struct Group {
	// The subscribe ID.
	pub subscribe: u64,

	// The group sequence number
	pub sequence: u64,
}

impl Decode for Group {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe: u64::decode_more(r, 1)?,
			sequence: u64::decode(r)?,
		})
	}
}

impl Encode for Group {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe.encode(w)?;
		self.sequence.encode(w)?;

		Ok(())
	}
}

#[derive(Clone, Debug, Copy)]
pub enum GroupOrder {
	Ascending,
	Descending,
}

impl Decode for GroupOrder {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0 => Ok(Self::Ascending),
			1 => Ok(Self::Descending),
			_ => Err(DecodeError::InvalidValue),
		}
	}
}

impl Encode for GroupOrder {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		let v: u64 = match self {
			Self::Ascending => 0,
			Self::Descending => 1,
		};
		v.encode(w)
	}
}

#[derive(Clone, Debug)]
pub struct GroupDrop {
	pub sequence: u64,
	pub count: u64,
	pub code: u32,
}

impl Encode for GroupDrop {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.sequence.encode(w)?;
		self.count.encode(w)?;
		self.code.encode(w)?;

		Ok(())
	}
}

impl Decode for GroupDrop {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			sequence: u64::decode_more(r, 2)?,
			count: u64::decode_more(r, 1)?,
			code: u32::decode(r)?,
		})
	}
}
