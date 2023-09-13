use crate::coding::{decode_string, encode_string, DecodeError, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to announce the availability of a group of tracks.
#[derive(Clone, Debug)]
pub struct Announce {
	// The track namespace
	pub namespace: String,
}

impl Announce {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = decode_string(r).await?;
		Ok(Self { namespace })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		encode_string(&self.namespace, w).await?;
		Ok(())
	}
}
