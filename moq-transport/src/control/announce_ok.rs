use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use bytes::{Buf, BufMut};

#[derive(Debug)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,
}

impl Decode for AnnounceOk {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_namespace = String::decode(r)?;
		Ok(Self { track_namespace })
	}
}

impl Encode for AnnounceOk {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_namespace.encode(w)
	}
}
