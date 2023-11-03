use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::setup::Extensions;

/// Sent by the publisher to accept a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209
	/// The ID for this track.
	pub id: VarInt,

	/// The subscription will expire in this many milliseconds.
	pub expires: VarInt,
}

impl SubscribeOk {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let expires = VarInt::decode(r).await?;
		Ok(Self { id, expires })
	}
}

impl SubscribeOk {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.expires.encode(w).await?;
		Ok(())
	}
}
