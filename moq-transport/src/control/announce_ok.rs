use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,
}

#[async_trait(?Send)]
impl Decode for AnnounceOk {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;
		Ok(Self { track_namespace })
	}
}

#[async_trait(?Send)]
impl Encode for AnnounceOk {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await
	}
}
