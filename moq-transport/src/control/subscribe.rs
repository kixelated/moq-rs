use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: u64,

	// The track namespace.
	pub track_namespace: String,

	// The track name.
	pub track_name: String,
}

#[async_trait]
impl Decode for Subscribe {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let track_id = u64::decode(r).await?;
		let track_namespace = String::decode(r).await?;
		let track_name = String::decode(r).await?;

		Ok(Self {
			track_id,
			track_namespace,
			track_name,
		})
	}
}

#[async_trait]
impl Encode for Subscribe {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_id.encode(w).await?;
		self.track_namespace.encode(w).await?;
		self.track_name.encode(w).await?;

		Ok(())
	}
}
