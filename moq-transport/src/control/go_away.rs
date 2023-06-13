use crate::coding::{Decode, Encode, Size};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct GoAway {
	pub url: String,
}

#[async_trait(?Send)]
impl Decode for GoAway {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let url = String::decode(r).await?;
		Ok(Self { url })
	}
}

#[async_trait(?Send)]
impl Encode for GoAway {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.url.encode(w).await
	}
}

impl Size for GoAway {
	fn size(&self) -> usize {
		self.url.size()
	}
}
