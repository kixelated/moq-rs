use crate::coding::{Decode, Encode, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use std::ops::Deref;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub VarInt);

impl Version {
	pub const DRAFT_00: Version = Version(VarInt::from_u32(0xff00));
}

impl From<VarInt> for Version {
	fn from(v: VarInt) -> Self {
		Self(v)
	}
}

impl From<Version> for VarInt {
	fn from(v: Version) -> Self {
		v.0
	}
}

#[async_trait]
impl Decode for Version {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let v = VarInt::decode(r).await?;
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
		let count = VarInt::decode(r).await?.into_inner();
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
		let size: VarInt = self.0.len().try_into()?;
		size.encode(w).await?;
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
