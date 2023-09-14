use crate::coding::{DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the subscriber to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeStop {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this subscription.
	pub id: VarInt,
}

impl SubscribeStop {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		Ok(Self { id })
	}
}

impl SubscribeStop {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		Ok(())
	}
}
