use crate::coding::{Decode, Duration, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub track_id: VarInt,

	// When non-zero, the subscription will end after this duration has elapsed.
	pub expires: Duration,
}

#[async_trait(?Send)]
impl Decode for SubscribeOk {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r).await?;
		let expires = Duration::decode(r).await?;

		Ok(Self { track_id, expires })
	}
}

#[async_trait(?Send)]
impl Encode for SubscribeOk {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_id.encode(w).await?;
		self.expires.encode(w).await?;

		Ok(())
	}
}

impl Size for SubscribeOk {
	fn size(&self) -> usize {
		self.track_id.size() + self.expires.size()
	}
}
