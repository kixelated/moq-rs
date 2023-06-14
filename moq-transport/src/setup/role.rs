use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::coding::{Decode, Encode};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
	Publisher,
	Subscriber,
	Both,
}

impl Role {
	pub fn is_publisher(&self) -> bool {
		match self {
			Self::Publisher | Self::Both => true,
			Self::Subscriber => false,
		}
	}

	pub fn is_subscriber(&self) -> bool {
		match self {
			Self::Subscriber | Self::Both => true,
			Self::Publisher => false,
		}
	}
}

impl From<Role> for u64 {
	fn from(r: Role) -> Self {
		match r {
			Role::Publisher => 0x0,
			Role::Subscriber => 0x1,
			Role::Both => 0x2,
		}
	}
}

impl TryFrom<u64> for Role {
	type Error = anyhow::Error;

	fn try_from(v: u64) -> Result<Self, Self::Error> {
		Ok(match v {
			0x0 => Self::Publisher,
			0x1 => Self::Subscriber,
			0x2 => Self::Both,
			_ => anyhow::bail!("invalid role: {}", v),
		})
	}
}

#[async_trait]
impl Decode for Role {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let v = u64::decode(r).await?;
		v.try_into()
	}
}

#[async_trait]
impl Encode for Role {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		u64::from(*self).encode(w).await
	}
}
