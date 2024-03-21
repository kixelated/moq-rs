use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the server to indicate that the client should connect to a different server.
#[derive(Clone, Debug)]
pub struct GoAway {
	pub url: String,
}

impl Decode for GoAway {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let url = String::decode(r)?;
		Ok(Self { url })
	}
}

impl Encode for GoAway {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.url.encode(w)
	}
}
