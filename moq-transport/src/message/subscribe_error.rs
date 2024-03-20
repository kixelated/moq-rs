use crate::coding::{AsyncRead, AsyncWrite};
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

impl SubscribeError {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r).await?;
		let code = u64::decode(r).await?;
		let reason = String::decode(r).await?;
		let alias = u64::decode(r).await?;

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
