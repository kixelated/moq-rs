use crate::coding::{Decode, Encode, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

// Another name for OBJECT, sent as a header for data streams.
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

#[async_trait]
impl Decode for Header {
	async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
		let typ = VarInt::decode(r).await?;
		anyhow::ensure!(u64::from(typ) == 0, "OBJECT type must be 0");

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

#[async_trait]
impl Encode for Header {
	async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
		VarInt::from_u32(0).encode(w).await?;
		self.track_id.encode(w).await?;
		self.group_sequence.encode(w).await?;
		self.object_sequence.encode(w).await?;
		self.send_order.encode(w).await?;

		Ok(())
	}
}
