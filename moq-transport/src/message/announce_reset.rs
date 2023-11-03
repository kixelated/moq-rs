use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::setup::Extensions;

/// Sent by the subscriber to reject an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceError {
	// Echo back the namespace that was reset
	pub namespace: String,

	// An error code.
	pub code: u32,

	// An optional, human-readable reason.
	pub reason: String,
}

impl AnnounceError {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;
		let code = VarInt::decode(r).await?.try_into()?;
		let reason = String::decode(r).await?;

		Ok(Self {
			namespace,
			code,
			reason,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.namespace.encode(w).await?;
		VarInt::from_u32(self.code).encode(w).await?;
		self.reason.encode(w).await?;

		Ok(())
	}
}
