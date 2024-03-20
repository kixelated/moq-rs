use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct ObjectHeader {
	// The subscribe ID.
	pub subscribe_id: u64,

	// The track alias.
	pub track_alias: u64,

	// The sequence number within the track.
	pub group_id: u64,

	// The sequence number within the group.
	pub object_id: u64,

	// The send order, where **smaller** values are sent first.
	pub send_order: u64,
}

impl ObjectHeader {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: u64::decode(r).await?,
			track_alias: u64::decode(r).await?,
			group_id: u64::decode(r).await?,
			object_id: u64::decode(r).await?,
			send_order: u64::decode(r).await?,
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
