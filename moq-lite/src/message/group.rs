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
			subscribe: u64::decode(r)?,
			sequence: u64::decode(r)?,
		})
	}
}

impl Encode for Group {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.subscribe.encode(w);
		self.sequence.encode(w);
	}
}

#[derive(Clone, Debug)]
pub struct GroupDrop {
	pub sequence: u64,
	pub count: u64,
	pub code: u32,
}

impl Encode for GroupDrop {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.sequence.encode(w);
		self.count.encode(w);
		self.code.encode(w);
	}
}

impl Decode for GroupDrop {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			sequence: u64::decode(r)?,
			count: u64::decode(r)?,
			code: u32::decode(r)?,
		})
	}
}
