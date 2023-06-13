use async_trait::async_trait;
use tokio::io::{AsyncWrite, AsyncWriteExt};

use super::VarInt;
use bytes::Bytes;

#[async_trait(?Send)]
pub trait Encode: Sized {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()>;
}

#[async_trait(?Send)]
impl Encode for Bytes {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.len().encode(w).await?;
		w.write_all(self).await?;
		Ok(())
	}
}

#[async_trait(?Send)]
impl Encode for Vec<u8> {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.len().encode(w).await?;
		w.write_all(self).await?;
		Ok(())
	}
}

#[async_trait(?Send)]
impl<T: Encode> Encode for Vec<T> {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.len().encode(w).await?;

		for item in self {
			item.encode(w).await?;
		}

		Ok(())
	}
}

#[async_trait(?Send)]
impl Encode for &[u8] {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.len().encode(w).await?;
		w.write_all(self).await?;
		Ok(())
	}
}

#[async_trait(?Send)]
impl Encode for String {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.as_bytes().encode(w).await
	}
}

#[async_trait(?Send)]
impl Encode for u64 {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		VarInt::try_from(*self)?.encode(w).await
	}
}

#[async_trait(?Send)]
impl Encode for usize {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		VarInt::try_from(*self)?.encode(w).await
	}
}
