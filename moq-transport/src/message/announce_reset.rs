use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the subscriber to reject an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceReset {
	// Echo back the namespace that was reset
	pub namespace: String,

	// An error code.
	pub code: u32,

	// An optional, human-readable reason.
	pub reason: String,
}

impl AnnounceReset {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = decode_string(r).await?;
		let code = VarInt::decode(r).await?.try_into()?;
		let reason = decode_string(r).await?;

		Ok(Self {
			namespace,
			code,
			reason,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.namespace, w).await?;
		VarInt::from_u32(self.code).encode(w).await?;
		encode_string(&self.reason, w).await?;

		Ok(())
	}
}
