use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

/// Sent by the publisher to reject a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeError {
	// The ID for this subscription.
	pub id: VarInt,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,

	/// An optional track alias, only used when error == Retry Track Alias
	pub alias: VarInt,
}

impl SubscribeError {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = String::decode(r).await?;
		let alias = VarInt::decode(r).await?;

		Ok(Self {
			id,
			code,
			reason,
			alias,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;
		self.alias.encode(w).await?;

		Ok(())
	}
}
