use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct Object {
	// The subscribe ID.
	pub subscribe: VarInt,

	// The track alias.
	pub track: VarInt,

	// The sequence number within the track.
	pub group: VarInt,

	// The sequence number within the group.
	pub sequence: VarInt,

	// The priority, where **smaller** values are sent first.
	pub priority: u32,
}

impl Object {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let subscribe = VarInt::decode(r).await?;
		let track = VarInt::decode(r).await?;
		let group = VarInt::decode(r).await?;
		let sequence = VarInt::decode(r).await?;
		let priority = VarInt::decode(r).await?.try_into()?;

		Ok(Self {
			subscribe,
			track,
			group,
			sequence,
			priority,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe.encode(w).await?;
		self.track.encode(w).await?;
		self.group.encode(w).await?;
		self.sequence.encode(w).await?;
		VarInt::from_u32(self.priority).encode(w).await?;

		Ok(())
	}
}
