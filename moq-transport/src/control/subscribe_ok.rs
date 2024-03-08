use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the publisher to accept a Subscribe.
#[derive(Clone, Debug)]
pub struct SubscribeOk {
	/// The ID for this subscription.
	pub id: VarInt,

	/// The subscription will expire in this many milliseconds.
	pub expires: Option<VarInt>,

	/// The latest group and object for the track.
	pub latest: Option<(VarInt, VarInt)>,
}

impl SubscribeOk {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let expires = match VarInt::decode(r).await? {
			VarInt::ZERO => None,
			expires => Some(expires),
		};

		let latest = match r.read_u8().await? {
			0 => None,
			1 => Some((VarInt::decode(r).await?, VarInt::decode(r).await?)),
			_ => return Err(DecodeError::InvalidValue),
		};

		Ok(Self { id, expires, latest })
	}
}

impl SubscribeOk {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		self.expires.unwrap_or(VarInt::ZERO).encode(w).await?;

		match self.latest {
			Some((group, object)) => {
				w.write_u8(1).await?;
				group.encode(w).await?;
				object.encode(w).await?;
			}
			None => {
				w.write_u8(0).await?;
			}
		}

		Ok(())
	}
}
