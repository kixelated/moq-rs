use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to announce the availability of a group of tracks.
#[derive(Clone, Debug)]
pub struct Announce {
	/// The track namespace
	pub namespace: String,

	/// An optional auth token.
	pub auth: String,
}

impl Announce {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;
		let auth = String::decode(r).await?;
		Ok(Self { namespace, auth })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.namespace.encode(w).await?;
		self.auth.encode(w).await?;
		Ok(())
	}
}
