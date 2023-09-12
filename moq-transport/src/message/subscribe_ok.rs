use crate::coding::{DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

#[derive(Clone, Debug)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub id: VarInt,

	// The subscription will end after this duration has elapsed.
	// A value of zero is invalid.
	pub expires: Option<VarInt>,
}

impl SubscribeOk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let expires = VarInt::decode(r).await?;
		let expires = if expires.into_inner() == 0 { None } else { Some(expires) };

		Ok(Self { id, expires })
	}
}

impl SubscribeOk {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.expires.unwrap_or_default().encode(w).await?;

		Ok(())
	}
}
