use crate::coding::{Decode, Encode, Params, Size};
use crate::version::Version;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// Sent by the client to setup up the session.
pub struct Client {
	// NOTE: This is not a message type, but rather the control stream header.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/138

	// The list of supported versions in preferred order.
	pub supported: Vec<Version>,

	// A generic list of paramters.
	pub params: Params,
}

#[async_trait(?Send)]
impl Decode for Client {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let supported = Vec::decode(r).await?;
		let params = Params::decode(r).await?;

		Ok(Self { supported, params })
	}
}

#[async_trait(?Send)]
impl Encode for Client {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.supported.encode(w).await?;
		self.params.encode(w).await?;

		Ok(())
	}
}

impl Size for Client {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.supported.size()? + self.params.size()?)
	}
}
