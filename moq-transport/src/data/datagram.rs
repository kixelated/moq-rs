use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct Datagram {
	// The subscribe ID.
	pub subscribe_id: VarInt,

	// The track alias.
	pub track_alias: VarInt,

	// The sequence number within the track.
	pub group_id: VarInt,

	// The object ID within the group.
	pub object_id: VarInt,

	// The priority, where **smaller** values are sent first.
	pub send_order: VarInt,
}

impl Datagram {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: VarInt::decode(r).await?,
			track_alias: VarInt::decode(r).await?,
			group_id: VarInt::decode(r).await?,
			object_id: VarInt::decode(r).await?,
			send_order: VarInt::decode(r).await?,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w).await?;
		self.track_alias.encode(w).await?;
		self.group_id.encode(w).await?;
		self.object_id.encode(w).await?;
		self.send_order.encode(w).await?;
		Ok(())
	}
}
