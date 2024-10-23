use crate::Path;

use super::{Decode, DecodeError, Encode};

impl Encode for Path {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) {
		self.len().encode(w);
		for part in self.as_ref() {
			part.encode(w);
		}
	}
}

impl Decode for Path {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let parts = Vec::<String>::decode(r)?;
		Ok(Path::new(parts))
	}
}
