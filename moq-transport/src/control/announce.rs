use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct Announce {
	// The track namespace
	pub track_namespace: String,
}

impl Decode for Announce {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_namespace = String::decode(r)?;
		Ok(Self { track_namespace })
	}
}

impl Encode for Announce {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_namespace.encode(w)?;
		Ok(())
	}
}
