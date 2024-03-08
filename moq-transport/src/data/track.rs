use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct Track {
	// The subscribe ID.
	pub subscribe_id: VarInt,

	// The track ID.
	pub track_alias: VarInt,

	// The priority, where **smaller** values are sent first.
	pub send_order: VarInt,
}

impl Track {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: VarInt::decode(r).await?,
			track_alias: VarInt::decode(r).await?,
			send_order: VarInt::decode(r).await?,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w).await?;
		self.track_alias.encode(w).await?;
		self.send_order.encode(w).await?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct TrackChunk {
	pub group_id: VarInt,
	pub object_id: VarInt,
	pub size: VarInt,
}

impl TrackChunk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let group_id = VarInt::decode(r).await?;
		let object_id = VarInt::decode(r).await?;
		let size = VarInt::decode(r).await?;

		Ok(Self {
			group_id,
			object_id,
			size,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.group_id.encode(w).await?;
		self.object_id.encode(w).await?;
		self.size.encode(w).await?;

		Ok(())
	}
}
