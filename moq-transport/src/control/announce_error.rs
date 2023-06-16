use crate::coding::{Decode, Encode, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct AnnounceError {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

#[async_trait]
impl Decode for AnnounceError {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = String::decode(r).await?;

		Ok(Self {
			track_namespace,
			code,
			reason,
		})
	}
}

#[async_trait]
impl Encode for AnnounceError {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;

		Ok(())
	}
}
