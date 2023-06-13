use super::VarInt;
use bytes::Bytes;
use std::str;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt};

#[async_trait(?Send)]
pub trait Decode: Sized {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self>;
}

#[async_trait(?Send)]
impl Decode for Bytes {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		Vec::<u8>::decode(r).await.map(Bytes::from)
	}
}

#[async_trait(?Send)]
impl Decode for Vec<u8> {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let size = u64::decode(r).await?;

		// NOTE: we don't use with_capacity since size is from an untrusted source
		let mut buf = Vec::new();
		r.take(size).read_to_end(&mut buf).await?;

		Ok(buf)
	}
}

#[async_trait(?Send)]
impl<T: Decode> Decode for Vec<T> {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let count = u64::decode(r).await?;

		// NOTE: we don't use with_capacity since count is from an untrusted source
		let mut v = Vec::new();

		for _ in 0..count {
			v.push(T::decode(r).await?);
		}

		Ok(v)
	}
}

#[async_trait(?Send)]
impl Decode for String {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let data = Vec::decode(r).await?;
		let s = str::from_utf8(&data)?.to_string();
		Ok(s)
	}
}

#[async_trait(?Send)]
impl Decode for u64 {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		VarInt::decode(r).await.map(Into::into)
	}
}

#[async_trait(?Send)]
impl Decode for usize {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		VarInt::decode(r).await.map(Into::into)
	}
}
