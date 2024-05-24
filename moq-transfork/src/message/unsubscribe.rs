use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the subscriber to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct Unsubscribe {
	// The ID for this subscription.
	pub id: u64,
}

impl Decode for Unsubscribe {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		Ok(Self { id })
	}
}

impl Encode for Unsubscribe {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		Ok(())
	}
}
