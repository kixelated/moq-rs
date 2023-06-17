use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct GoAway {
	pub url: String,
}

#[async_trait]
impl Decode for GoAway {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let url = String::decode(r).await?;
		Ok(Self { url })
	}
}

#[async_trait]
impl Encode for GoAway {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.url.encode(w).await
	}
}
