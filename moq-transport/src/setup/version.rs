use crate::coding::{Decode, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version(pub VarInt);

impl Version {
	pub const DRAFT_00: Version = Version(VarInt(0xff00));
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

#[async_trait(?Send)]
impl Decode for Version {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let v = VarInt::decode(r).await?;
		Ok(Self(v))
	}
}

#[async_trait(?Send)]
impl Encode for Version {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.0.encode(w).await
	}
}

impl Size for Version {
	fn size(&self) -> anyhow::Result<usize> {
		self.0.size()
	}
}
