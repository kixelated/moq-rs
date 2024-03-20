use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError};

/// Sent by the publisher to cleanly terminate a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeDone {
	/// The ID for this subscription.
	pub id: u64,

	/// The error code
	pub code: u64,

	/// An optional error reason
	pub reason: String,

	/// The final group/object sent on this subscription.
	pub last: Option<(u64, u64)>,
}

impl SubscribeDone {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r).await?;
		let code = u64::decode(r).await?;
		let reason = String::decode(r).await?;
		let last = match r.read_u8().await.map_err(|_| DecodeError::IoError)? {
			0 => None,
			1 => Some((u64::decode(r).await?, u64::decode(r).await?)),
			_ => return Err(DecodeError::InvalidValue),
		};

		Ok(Self { id, code, reason, last })
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.code.encode(w).await?;
		self.reason.encode(w).await?;

		if let Some((group, object)) = self.last {
			w.write_u8(1).await.map_err(|_| EncodeError::IoError)?;
			group.encode(w).await?;
			object.encode(w).await?;
		} else {
			w.write_u8(0).await.map_err(|_| EncodeError::IoError)?;
		}

		Ok(())
	}
}
