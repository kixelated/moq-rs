use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

/// Sent by the publisher to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeReset {
	/// The ID for this subscription.
	pub id: VarInt,

	/// An error code.
	pub code: u32,

	/// An optional, human-readable reason.
	pub reason: String,

	/// The final group/object sent on this subscription.
	pub group: VarInt,
	pub object: VarInt,
}

impl SubscribeReset {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?.try_into()?;
		let reason = String::decode(r).await?;
		let group = VarInt::decode(r).await?;
		let object = VarInt::decode(r).await?;

		Ok(Self {
			id,
			code,
			reason,
			group,
			object,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		VarInt::from_u32(self.code).encode(w).await?;
		self.reason.encode(w).await?;

		self.group.encode(w).await?;
		self.object.encode(w).await?;

		Ok(())
	}
}
