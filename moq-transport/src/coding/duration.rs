use crate::coding::{Decode, Encode, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use std::time::Duration;

#[async_trait]
impl Encode for Duration {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		let ms = self.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.encode(w).await
	}
}

#[async_trait]
impl Decode for Duration {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let ms = VarInt::decode(r).await?;
		Ok(Self::from_millis(ms.into()))
	}
}
