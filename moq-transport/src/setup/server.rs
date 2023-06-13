use crate::coding::{Decode, Encode, Params, Size};
use crate::version::Version;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// Sent by the server in response to a client.
// NOTE: This is not a message type, but rather the control stream header.
// Proposal: https://github.com/moq-wg/moq-transport/issues/138
pub struct Server {
	// The list of supported versions in preferred order.
	pub selected: Version,

	// A generic list of paramters.
	pub params: Params,
}

#[async_trait(?Send)]
impl Decode for Server {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let selected = Version::decode(r).await?;
		let params = Params::decode(r).await?;

		Ok(Self { selected, params })
	}
}

#[async_trait(?Send)]
impl Encode for Server {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.selected.encode(w).await?;
		self.params.encode(w).await?;

		Ok(())
	}
}

impl Size for Server {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.selected.size()? + self.params.size()?)
	}
}
