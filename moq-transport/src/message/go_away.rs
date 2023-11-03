use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the server to indicate that the client should connect to a different server.
#[derive(Clone, Debug)]
pub struct GoAway {
	pub url: String,
}

impl GoAway {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let url = String::decode(r).await?;
		Ok(Self { url })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.url.encode(w).await
	}
}
