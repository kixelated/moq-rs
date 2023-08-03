use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct GoAway {
	pub url: String,
}

impl Decode for GoAway {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let url = String::decode(r)?;
		Ok(Self { url })
	}
}

impl Encode for GoAway {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.url.encode(w)
	}
}
