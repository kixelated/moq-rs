use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::coding::{Decode, DecodeError, Encode, EncodeError};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to accept a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeOk {
	/// The ID for this subscription.
	pub id: u64,

	/// The subscription will expire in this many milliseconds.
	pub expires: Option<u64>,

	/// The latest group and object for the track.
	pub latest: Option<(u64, u64)>,
}

impl SubscribeOk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = u64::decode(r).await?;
		let expires = match u64::decode(r).await? {
			0 => None,
			expires => Some(expires),
		};

		let latest = match r.read_u8().await.map_err(|_| DecodeError::IoError)? {
			0 => None,
			1 => Some((u64::decode(r).await?, u64::decode(r).await?)),
			_ => return Err(DecodeError::InvalidValue),
		};

		Ok(Self { id, expires, latest })
	}
}

impl SubscribeOk {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.expires.unwrap_or(0).encode(w).await?;

		match self.latest {
			Some((group, object)) => {
				w.write_u8(1).await.map_err(|_| EncodeError::IoError)?;
				group.encode(w).await?;
				object.encode(w).await?;
			}
			None => {
				w.write_u8(0).await.map_err(|_| EncodeError::IoError)?;
			}
		}

		Ok(())
	}
}
