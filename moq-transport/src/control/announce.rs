use crate::coding::{Decode, Encode};
use bytes::Bytes;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use anyhow::Context;

#[derive(Debug)]
pub struct Announce {
	// The track namespace
	pub track_namespace: String,

	// An authentication token, param 0x02
	pub auth: Option<Bytes>,
}

#[async_trait]
impl Decode for Announce {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;

		let mut auth = None;

		while let Ok(id) = u64::decode(r).await {
			match id {
				0x2 => {
					let v = Bytes::decode(r).await.context("failed to decode auth")?;
					anyhow::ensure!(auth.replace(v).is_none(), "duplicate auth param");
				}
				_ => {
					anyhow::bail!("unknown param: {}", id);
				}
			}
		}

		Ok(Self { track_namespace, auth })
	}
}

#[async_trait]
impl Encode for Announce {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await?;

		if let Some(auth) = &self.auth {
			2u64.encode(w).await?;
			auth.encode(w).await?;
		}

		Ok(())
	}
}
