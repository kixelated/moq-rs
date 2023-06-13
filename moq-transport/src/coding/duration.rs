use super::{Decode, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use std::time;

#[derive(Default, Debug)]
pub struct Duration(pub time::Duration);

#[async_trait(?Send)]
impl Encode for Duration {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		let ms = self.0.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.encode(w).await
	}
}

#[async_trait(?Send)]
impl Decode for Duration {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let ms = VarInt::decode(r).await?;
		let ms = ms.into();
		Ok(Self(time::Duration::from_millis(ms)))
	}
}

impl Size for Duration {
	fn size(&self) -> anyhow::Result<usize> {
		let ms = self.0.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.size()
	}
}

impl From<Duration> for time::Duration {
	fn from(d: Duration) -> Self {
		d.0
	}
}

impl From<time::Duration> for Duration {
	fn from(d: time::Duration) -> Self {
		Self(d)
	}
}
