use super::{Role, Versions};
use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

use anyhow::Context;

// Sent by the client to setup up the session.
#[derive(Debug)]
pub struct Client {
	// NOTE: This is not a message type, but rather the control stream header.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/138

	// The list of supported versions in preferred order.
	pub versions: Versions,

	// Indicate if the client is a publisher, a subscriber, or both.
	// Proposal: moq-wg/moq-transport#151
	pub role: Role,

	// The path, non-empty ONLY when not using WebTransport.
	pub path: String,
}

#[async_trait]
impl Decode for Client {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let typ = u64::decode(r).await.context("failed to read type")?;
		anyhow::ensure!(typ == 1, "client SETUP must be type 1");

		let versions = Versions::decode(r).await.context("failed to read supported versions")?;
		anyhow::ensure!(!versions.is_empty(), "client must support at least one version");

		let role = Role::decode(r).await.context("failed to decode role")?;
		let path = String::decode(r).await.context("failed to read path")?;

		Ok(Self { versions, role, path })
	}
}

#[async_trait]
impl Encode for Client {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		1u64.encode(w).await?;

		anyhow::ensure!(!self.versions.is_empty(), "client must support at least one version");
		self.versions.encode(w).await?;
		self.role.encode(w).await?;
		self.path.encode(w).await?;

		Ok(())
	}
}
