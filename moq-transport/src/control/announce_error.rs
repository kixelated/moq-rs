use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the subscriber to reject an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceError {
	// Echo back the namespace that was reset
	pub namespace: String,

	// An error code.
	pub code: u64,

	// An optional, human-readable reason.
	pub reason: String,
}

impl AnnounceError {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;
		let code = u64::decode(r).await?;
		let reason = String::decode(r).await?;

		Ok(Self {
			namespace,
			code,
			reason,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.namespace.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;

		Ok(())
	}
}
