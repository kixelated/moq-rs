use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct GroupHeader {
	// The subscribe ID.
	pub subscribe: VarInt,

	// The track alias.
	pub track: VarInt,

	// The group sequence number
	pub group: VarInt,

	// The priority, where **smaller** values are sent first.
	pub priority: u32,
}

impl GroupHeader {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let subscribe = VarInt::decode(r).await?;
		let track = VarInt::decode(r).await?;
		let group = VarInt::decode(r).await?;
		let priority = VarInt::decode(r).await?.try_into()?;

		Ok(Self {
			subscribe,
			track,
			group,
			priority,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe.encode(w).await?;
		self.track.encode(w).await?;
		self.group.encode(w).await?;
		VarInt::from_u32(self.priority).encode(w).await?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct GroupChunk {
	pub object: VarInt,
	pub size: VarInt,
}

impl GroupChunk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let object = VarInt::decode(r).await?;
		let size = VarInt::decode(r).await?;

		Ok(Self { object, size })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.object.encode(w).await?;
		self.size.encode(w).await?;

		Ok(())
	}
}
