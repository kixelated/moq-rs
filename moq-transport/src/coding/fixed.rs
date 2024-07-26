use super::{Decode, DecodeError, Encode, EncodeError};

impl Encode for u8 {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		w.put_u8(*self);
		Ok(())
	}
}

impl Decode for u8 {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(r.get_u8())
	}
}
