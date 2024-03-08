use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

/// Sent by the subscriber to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct Unsubscribe {
	// The ID for this subscription.
	pub id: VarInt,
}

impl Unsubscribe {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		Ok(Self { id })
	}
}

impl Unsubscribe {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		Ok(())
	}
}
