use super::{Decode, DecodeError, Encode, EncodeError};

impl Encode for String {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.len().encode(w)?;
		Self::encode_remaining(w, self.len())?;
		w.put(self.as_ref());
		Ok(())
	}
}

impl Decode for String {
	/// Decode a string with a varint length prefix.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let size = usize::decode(r)?;
		Self::decode_remaining(r, size)?;

		let mut buf = vec![0; size];
		r.copy_to_slice(&mut buf);
		let str = String::from_utf8(buf)?;

		Ok(str)
	}
}
