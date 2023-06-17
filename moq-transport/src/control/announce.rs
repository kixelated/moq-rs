use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct Announce {
	// The track namespace
	pub track_namespace: String,
}

#[async_trait]
impl Decode for Announce {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;
		Ok(Self { track_namespace })
	}
}

#[async_trait]
impl Encode for Announce {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await?;
		Ok(())
	}
}
