use crate::coding::{Decode, Encode, VarInt};

use std::time::Duration;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub track_id: VarInt,

	// The subscription will end after this duration has elapsed.
	// A value of zero is invalid.
	pub expires: Option<Duration>,
}

#[async_trait]
impl Decode for SubscribeOk {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r).await?;
		let expires = Duration::decode(r).await?;
		let expires = if expires == Duration::ZERO { None } else { Some(expires) };

		Ok(Self { track_id, expires })
	}
}

#[async_trait]
impl Encode for SubscribeOk {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_id.encode(w).await?;
		self.expires.unwrap_or_default().encode(w).await?;

		Ok(())
	}
}
