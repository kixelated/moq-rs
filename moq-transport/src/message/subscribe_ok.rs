use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to accept a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeOk {
	/// The ID for this subscription.
	pub id: VarInt,

	/// The subscription will expire in this many milliseconds.
	pub expires: Option<VarInt>,
}

impl SubscribeOk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let expires = match VarInt::decode(r).await? {
			VarInt::ZERO => None,
			expires => Some(expires),
		};
		Ok(Self { id, expires })
	}
}

impl SubscribeOk {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.expires.unwrap_or(VarInt::ZERO).encode(w).await?;
		Ok(())
	}
}
