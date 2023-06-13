use crate::coding::{Decode, Encode, Params};
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

	// Parameters that we don't recognize.
	pub unknown: Params,
}

#[async_trait(?Send)]
impl Decode for Announce {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;

		let mut auth = None;
		let mut unknown = Params::new();

		while let Ok(id) = u64::decode(r).await {
			match id {
				0x2 => {
					let v = Bytes::decode(r).await.context("failed to decode auth")?;
					anyhow::ensure!(auth.replace(v).is_none(), "duplicate auth param");
				}
				_ => {
					unknown
						.decode_one(id, r)
						.await
						.context("failed to decode unknown param")?;
				}
			}
		}

		Ok(Self {
			track_namespace,
			auth,
			unknown,
		})
	}
}

#[async_trait(?Send)]
impl Encode for Announce {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await?;

		if let Some(auth) = &self.auth {
			2u64.encode(w).await?;
			auth.encode(w).await?;
		}

		self.unknown.encode(w).await?;

		Ok(())
	}
}
