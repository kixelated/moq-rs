use crate::coding::{decode_string, encode_string, DecodeError, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to terminate an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceStop {
	// Echo back the namespace that was reset
	pub namespace: String,
}

impl AnnounceStop {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = decode_string(r).await?;

		Ok(Self { namespace })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.namespace, w).await?;

		Ok(())
	}
}
