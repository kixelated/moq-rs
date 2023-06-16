use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub u64);

impl Version {
	pub const DRAFT_00: Version = Version(0xff00);
}

impl From<u64> for Version {
	fn from(v: u64) -> Self {
		Self(v)
	}
}

impl From<Version> for u64 {
	fn from(v: Version) -> Self {
		v.0
	}
}

#[async_trait]
impl Decode for Version {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let v = u64::decode(r).await?;
		Ok(Self(v))
	}
}

#[async_trait]
impl Encode for Version {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.0.encode(w).await
	}
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Versions(pub Vec<Version>);

#[async_trait]
impl Decode for Versions {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let count = u64::decode(r).await?;
		let mut vs = Vec::new();

		for _ in 0..count {
			let v = Version::decode(r).await?;
			vs.push(v);
		}

		Ok(Self(vs))
	}
}

#[async_trait]
impl Encode for Versions {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		self.0.len().encode(w).await?;
		for v in &self.0 {
			v.encode(w).await?;
		}
		Ok(())
	}
}

impl Deref for Versions {
	type Target = Vec<Version>;

	fn deref(&self) -> &Self::Target {
		&self.0
	}
}
