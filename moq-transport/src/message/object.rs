use std::time;

use crate::coding::{DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

/// Sent by the publisher as the header of each data stream.
#[derive(Clone, Debug)]
pub struct Object {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track: VarInt,

	// The sequence number within the track.
	pub sequence: VarInt,

	// The priority, where **larger** values are sent first.
	// Proposal: int32 instead of a varint.
	pub priority: i32,

	// Cache the object for at most this many seconds.
	// Zero means never expire.
	pub expires: Option<time::Duration>,
}

impl Object {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let typ = VarInt::decode(r).await?;
		if typ.into_inner() != 0 {
			return Err(DecodeError::InvalidType(typ));
		}

		// NOTE: size has been omitted

		let track = VarInt::decode(r).await?;
		let sequence = VarInt::decode(r).await?;
		let priority = r.read_i32().await?; // big-endian
		let expires = match VarInt::decode(r).await?.into_inner() {
			0 => None,
			secs => Some(time::Duration::from_secs(secs)),
		};

		Ok(Self {
			track,
			sequence,
			priority,
			expires,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		VarInt::ZERO.encode(w).await?;
		self.track.encode(w).await?;
		self.sequence.encode(w).await?;
		w.write_i32(self.priority).await?;

		// Round up if there's any decimal points.
		let expires = match self.expires {
			None => 0,
			Some(time::Duration::ZERO) => return Err(EncodeError::InvalidValue), // there's no way of expressing zero currently.
			Some(expires) if expires.subsec_nanos() > 0 => expires.as_secs() + 1,
			Some(expires) => expires.as_secs(),
		};

		VarInt::try_from(expires)?.encode(w).await?;

		Ok(())
	}
}
