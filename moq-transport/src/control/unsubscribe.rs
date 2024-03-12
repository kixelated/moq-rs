use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the subscriber to terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct Unsubscribe {
	// The ID for this subscription.
	pub id: u64,
}

impl Unsubscribe {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r).await?;
		Ok(Self { id })
	}
}

impl Unsubscribe {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		Ok(())
	}
}
