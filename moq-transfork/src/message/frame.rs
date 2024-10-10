use crate::coding::*;

#[derive(Clone, Debug)]
pub struct Frame {
	pub size: usize,
}

impl Decode for Frame {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			size: usize::decode(r)?,
		})
	}
}

impl Encode for Frame {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.size.encode(w);
	}
}
