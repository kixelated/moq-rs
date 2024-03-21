use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the publisher to reject a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeError {
	// The ID for this subscription.
	pub id: u64,

	// An error code.
	pub code: u64,

	// An optional, human-readable reason.
	pub reason: String,

	/// An optional track alias, only used when error == Retry Track Alias
	pub alias: u64,
}

impl Decode for SubscribeError {
	fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r)?;
		let code = u64::decode(r)?;
		let reason = String::decode(r)?;
		let alias = u64::decode(r)?;

		Ok(Self {
			id,
			code,
			reason,
			alias,
		})
	}
}

impl Encode for SubscribeError {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;
		self.alias.encode(w)?;

		Ok(())
	}
}
