use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};
use crate::setup::Extensions;

/// Sent by the publisher to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeReset {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209
	/// The ID for this subscription.
	pub id: VarInt,

	/// An error code.
	pub code: u32,

	/// An optional, human-readable reason.
	pub reason: String,

	/// The final group/object sent on this subscription.
	pub final_group: VarInt,
	pub final_object: VarInt,
}

impl SubscribeReset {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?.try_into()?;
		let reason = String::decode(r).await?;
		let final_group = VarInt::decode(r).await?;
		let final_object = VarInt::decode(r).await?;

		Ok(Self {
			id,
			code,
			reason,
			final_group,
			final_object,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		VarInt::from_u32(self.code).encode(w).await?;
		self.reason.encode(w).await?;

		self.final_group.encode(w).await?;
		self.final_object.encode(w).await?;

		Ok(())
	}
}
