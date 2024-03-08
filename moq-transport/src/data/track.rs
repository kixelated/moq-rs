use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

#[derive(Clone, Debug)]
pub struct Track {
	// The subscribe ID.
	pub subscribe: VarInt,

	// The track ID.
	pub track: VarInt,

	// The priority, where **smaller** values are sent first.
	pub priority: u32,
}

impl Track {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let subscribe = VarInt::decode(r).await?;
		let track = VarInt::decode(r).await?;
		let priority = VarInt::decode(r).await?.try_into()?;

		Ok(Self {
			subscribe,
			track,
			priority,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe.encode(w).await?;
		self.track.encode(w).await?;
		VarInt::from_u32(self.priority).encode(w).await?;

		Ok(())
	}
}

#[derive(Clone, Debug)]
pub struct TrackChunk {
	pub group: VarInt,
	pub object: VarInt,
	pub size: VarInt,
}

impl TrackChunk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let group = VarInt::decode(r).await?;
		let object = VarInt::decode(r).await?;
		let size = VarInt::decode(r).await?;

		Ok(Self { group, object, size })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.group.encode(w).await?;
		self.object.encode(w).await?;
		self.size.encode(w).await?;

		Ok(())
	}
}
