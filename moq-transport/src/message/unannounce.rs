use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to terminate an Announce.
#[derive(Clone, Debug)]
pub struct Unannounce {
	// Echo back the namespace that was reset
	pub namespace: String,
}

impl Unannounce {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;

		Ok(Self { namespace })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.namespace.encode(w).await?;

		Ok(())
	}
}
