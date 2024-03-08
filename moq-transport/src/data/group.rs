use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct Group {
	// The subscribe ID.
	pub subscribe_id: VarInt,

	// The track alias.
	pub track_alias: VarInt,

	// The group sequence number
	pub group_id: VarInt,

	// The priority, where **smaller** values are sent first.
	pub send_order: VarInt,
}

impl Group {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: VarInt::decode(r).await?,
			track_alias: VarInt::decode(r).await?,
			group_id: VarInt::decode(r).await?,
			send_order: VarInt::decode(r).await?,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w).await?;
		self.track_alias.encode(w).await?;
		self.group_id.encode(w).await?;
		self.send_order.encode(w).await?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct GroupChunk {
	pub object_id: VarInt,
	pub size: VarInt,
}

impl GroupChunk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			object_id: VarInt::decode(r).await?,
			size: VarInt::decode(r).await?,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.object_id.encode(w).await?;
		self.size.encode(w).await?;

		Ok(())
	}
}
