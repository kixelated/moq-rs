use crate::coding::{Decode, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct SubscribeError {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub track_id: VarInt,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

#[async_trait(?Send)]
impl Decode for SubscribeError {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = String::decode(r).await?;

		Ok(Self { track_id, code, reason })
	}
}

#[async_trait(?Send)]
impl Encode for SubscribeError {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_id.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;

		Ok(())
	}
}

impl Size for SubscribeError {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()? + self.code.size()? + self.reason.size()?)
	}
}
