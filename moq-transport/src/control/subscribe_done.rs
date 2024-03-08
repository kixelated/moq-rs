use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

/// Sent by the publisher to cleanly terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeDone {
	/// The ID for this subscription.
	pub id: VarInt,

	/// The error code
	pub code: VarInt,

	/// An optional error reason
	pub reason: String,

	/// The final group/object sent on this subscription.
	pub last: Option<(VarInt, VarInt)>,
}

impl SubscribeDone {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = String::decode(r).await?;
		let last = match r.read_u8().await? {
			0 => None,
			1 => Some((VarInt::decode(r).await?, VarInt::decode(r).await?)),
			_ => return Err(DecodeError::InvalidValue),
		};

		Ok(Self { id, code, reason, last })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;

		if let Some((group, object)) = self.last {
			w.write_u8(1).await?;
			group.encode(w).await?;
			object.encode(w).await?;
		} else {
			w.write_u8(0).await?;
		}

		Ok(())
	}
}
