use super::{Decode, DecodeError, Encode, EncodeError};

impl Encode for String {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.len().encode(w)?;
		if w.remaining_mut() < self.len() {
			return Err(EncodeError::More(self.len()));
		}

		w.put(self.as_ref());
		Ok(())
	}
}

impl Decode for String {
	/// Decode a string with a varint length prefix.
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let size = usize::decode(r)?;
		if r.remaining() < size {
			return Err(DecodeError::More(size));
		}

		let mut buf = vec![0; size];
		r.copy_to_slice(&mut buf);
		let str = String::from_utf8(buf)?;

		Ok(str)
	}
}
