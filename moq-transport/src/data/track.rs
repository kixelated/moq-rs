use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

#[derive(Clone, Debug)]
pub struct TrackHeader {
	// The subscribe ID.
	pub subscribe_id: u64,

	// The track ID.
	pub track_alias: u64,

	// The priority, where **smaller** values are sent first.
	pub send_order: u64,
}

impl TrackHeader {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		Ok(Self {
			subscribe_id: u64::decode(r).await?,
			track_alias: u64::decode(r).await?,
			send_order: u64::decode(r).await?,
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
pub struct TrackObject {
	pub group_id: u64,
	pub object_id: u64,
	pub size: usize,
}

impl TrackObject {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Option<Self>, DecodeError> {
		let group_id = match u64::decode(r).await {
			Ok(group_id) => group_id,
			Err(DecodeError::UnexpectedEnd) => return Ok(None),
			Err(err) => return Err(err),
		};

		let object_id = u64::decode(r).await?;
		let size = usize::decode(r).await?;

		Ok(Some(Self {
			group_id,
			object_id,
			size,
		}))
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.group_id.encode(w).await?;
		self.object_id.encode(w).await?;
		self.size.encode(w).await?;

		Ok(())
	}
}
