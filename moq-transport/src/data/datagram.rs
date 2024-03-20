use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

#[derive(Clone, Debug)]
pub struct Datagram {
	// The subscribe ID.
	pub subscribe_id: u64,

	// The track alias.
	pub track_alias: u64,

	// The sequence number within the track.
	pub group_id: u64,

	// The object ID within the group.
	pub object_id: u64,

	// The priority, where **smaller** values are sent first.
	pub send_order: u64,

	// The payload.
	pub payload: bytes::Bytes,
}

impl Datagram {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let subscribe_id = u64::decode(r).await?;
		let track_alias = u64::decode(r).await?;
		let group_id = u64::decode(r).await?;
		let object_id = u64::decode(r).await?;
		let send_order = u64::decode(r).await?;

		// TODO use with_capacity once we know the size of the datagram...
		let mut payload = Vec::new();
		r.read_to_end(&mut payload).await.map_err(|_| DecodeError::IoError)?;

		Ok(Self {
			subscribe_id,
			track_alias,
			group_id,
			object_id,
			send_order,
			payload: payload.into(),
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.subscribe_id.encode(w).await?;
		self.track_alias.encode(w).await?;
		self.group_id.encode(w).await?;
		self.object_id.encode(w).await?;
		self.send_order.encode(w).await?;
		w.write_all(self.payload.as_ref())
			.await
			.map_err(|_| EncodeError::IoError)?;
		Ok(())
	}
}
