use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// Another name for OBJECT, sent as a header for data streams.
#[derive(Debug)]
pub struct Header {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: u64,

	// The group sequence number.
	pub group_sequence: u64,

	// The object sequence number.
	pub object_sequence: u64,

	// The priority/send order.
	pub send_order: u64,
}

#[async_trait]
impl Decode for Header {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let typ = u64::decode(r).await?;
		anyhow::ensure!(typ == 0, "OBJECT type must be 0");

		// NOTE: size has been omitted

		let track_id = u64::decode(r).await?;
		let group_sequence = u64::decode(r).await?;
		let object_sequence = u64::decode(r).await?;
		let send_order = u64::decode(r).await?;

		Ok(Self {
			track_id,
			group_sequence,
			object_sequence,
			send_order,
		})
	}
}

#[async_trait]
impl Encode for Header {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		0u64.encode(w).await?;
		self.track_id.encode(w).await?;
		self.group_sequence.encode(w).await?;
		self.object_sequence.encode(w).await?;
		self.send_order.encode(w).await?;

		Ok(())
	}
}
