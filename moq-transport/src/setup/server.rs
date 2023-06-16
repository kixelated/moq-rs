use super::{Role, Version};
use crate::coding::{Decode, Encode};

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
}

#[async_trait]
impl Decode for Server {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let typ = u64::decode(r).await.context("failed to read type")?;
		anyhow::ensure!(typ == 2, "server SETUP must be type 2");

		let version = Version::decode(r).await.context("failed to read version")?;
		let role = Role::decode(r).await.context("failed to read role")?;

		Ok(Self { version, role })
	}
}

#[async_trait]
impl Encode for Server {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		2u64.encode(w).await?; // setup type

		self.version.encode(w).await?;
		self.role.encode(w).await?;

		Ok(())
	}
}
