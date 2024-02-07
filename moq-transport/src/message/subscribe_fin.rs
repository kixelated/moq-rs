use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

/// Sent by the publisher to cleanly terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeFin {
	/// The ID for this subscription.
	pub id: VarInt,

	/// The final group/object sent on this subscription.
	pub group: VarInt,
	pub object: VarInt,
}

impl SubscribeFin {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let group = VarInt::decode(r).await?;
		let object = VarInt::decode(r).await?;

		Ok(Self { id, group, object })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.group.encode(w).await?;
		self.object.encode(w).await?;

		Ok(())
	}
}
