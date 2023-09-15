use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to reject a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeReset {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this subscription.
	pub id: VarInt,

	// An error code.
	pub code: u32,

	// An optional, human-readable reason.
	pub reason: String,
}

impl SubscribeReset {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?.try_into()?;
		let reason = decode_string(r).await?;

		Ok(Self { id, code, reason })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		VarInt::from_u32(self.code).encode(w).await?;
		encode_string(&self.reason, w).await?;

		Ok(())
	}
}
