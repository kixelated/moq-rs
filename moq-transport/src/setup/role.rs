use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use crate::coding::{Decode, Encode, Size, VarInt};

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

impl From<Role> for VarInt {
	fn from(r: Role) -> Self {
		VarInt(match r {
			Role::Publisher => 0x0,
			Role::Subscriber => 0x1,
			Role::Both => 0x2,
		})
	}
}

impl TryFrom<VarInt> for Role {
	type Error = anyhow::Error;

	fn try_from(v: VarInt) -> Result<Self, Self::Error> {
		Ok(match v {
			VarInt(0x0) => Self::Publisher,
			VarInt(0x1) => Self::Subscriber,
			VarInt(0x2) => Self::Both,
			_ => anyhow::bail!("invalid role: {}", v),
		})
	}
}

#[async_trait(?Send)]
impl Decode for Role {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let v = VarInt::decode(r).await?;
		v.try_into()
	}
}

#[async_trait(?Send)]
impl Encode for Role {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		VarInt::from(*self).encode(w).await
	}
}

impl Size for Role {
	fn size(&self) -> anyhow::Result<usize> {
		VarInt::from(*self).size()
	}
}
