use crate::coding::{Decode, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// This is the header for a data stream, aka an OBJECT.
#[derive(Debug)]
pub struct Header {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The group sequence number.
	pub group_sequence: VarInt,

	// The object sequence number.
	pub object_sequence: VarInt,

	// The priority/send order.
	pub send_order: VarInt,
}

#[async_trait(?Send)]
impl Decode for Header {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let typ = VarInt::decode(r).await?;
		anyhow::ensure!(typ == VarInt(0), "typ must be 0");

		// NOTE: size has been omitted

		let track_id = VarInt::decode(r).await?;
		let group_sequence = VarInt::decode(r).await?;
		let object_sequence = VarInt::decode(r).await?;
		let send_order = VarInt::decode(r).await?;

		Ok(Self {
			track_id,
			group_sequence,
			object_sequence,
			send_order,
		})
	}
}

#[async_trait(?Send)]
impl Encode for Header {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		VarInt(0).encode(w).await?;
		self.track_id.encode(w).await?;
		self.group_sequence.encode(w).await?;
		self.object_sequence.encode(w).await?;
		self.send_order.encode(w).await?;

		Ok(())
	}
}

impl Size for Header {
	fn size(&self) -> usize {
		VarInt(0).size()
			+ self.track_id.size()
			+ self.group_sequence.size()
			+ self.object_sequence.size()
			+ self.send_order.size()
	}
}
