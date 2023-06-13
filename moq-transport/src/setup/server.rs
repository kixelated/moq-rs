use super::{Role, Version};
use crate::coding::{Decode, Encode, Params, Size, VarInt};

use anyhow::Context;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// Sent by the server in response to a client.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
#[derive(Debug)]
pub struct Server {
	// The list of supported versions in preferred order.
	pub version: Version,

	// param: 0x0: Indicate if the server is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,

	// A list of unknown paramters.
	pub unknown: Params,
}

#[async_trait(?Send)]
impl Decode for Server {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let version = Version::decode(r).await?;

		let mut role = None;
		let mut unknown = Params::new();

		while let Ok(id) = VarInt::decode(r).await {
			match id {
				VarInt(0x0) => {
					let v = Role::decode(r).await.context("failed to decode role")?;
					anyhow::ensure!(role.replace(v).is_none(), "duplicate role parameter");
				}
				VarInt(0x1) => {
					anyhow::bail!("server must not send path parameter");
				}
				_ => {
					unknown
						.decode_one(id, r)
						.await
						.context("failed to decode unknown param")?;
				}
			};
		}

		let role = role.context("missing role parameter")?;

		Ok(Self { version, role, unknown })
	}
}

#[async_trait(?Send)]
impl Encode for Server {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.version.encode(w).await?;
		VarInt(0x0).encode(w).await?;
		self.role.encode(w).await?;
		self.unknown.encode(w).await?;

		Ok(())
	}
}

impl Size for Server {
	fn size(&self) -> usize {
		let mut size = self.version.size() + self.unknown.size();
		size += VarInt(0x0).size() + self.role.size();

		size
	}
}
