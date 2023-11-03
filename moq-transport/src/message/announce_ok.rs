use crate::{
	coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError},
	setup::Extensions,
};

/// Sent by the subscriber to accept an Announce.
#[derive(Clone, Debug)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub namespace: String,
}

impl AnnounceOk {
	pub async fn decode<R: AsyncRead>(r: &mut R, _ext: &Extensions) -> Result<Self, DecodeError> {
		let namespace = String::decode(r).await?;
		Ok(Self { namespace })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, _ext: &Extensions) -> Result<(), EncodeError> {
		self.namespace.encode(w).await
	}
}
