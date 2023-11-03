use std::{io, time};

use tokio::io::AsyncReadExt;

use crate::coding::{AsyncRead, AsyncWrite};
use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};
use crate::setup;

/// Sent by the publisher as the header of each data stream.
#[derive(Clone, Debug)]
pub struct Object {
	// An ID for this track.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track: VarInt,

	// The sequence number within the track.
	pub group: VarInt,

	// The sequence number within the group.
	pub sequence: VarInt,

	// The priority, where **smaller** values are sent first.
	pub priority: u32,

	// Cache the object for at most this many seconds.
	// Zero means never expire.
	pub expires: Option<time::Duration>,

	/// An optional size, allowing multiple OBJECTs on the same stream.
	pub size: Option<VarInt>,
}

impl Object {
	pub async fn decode<R: AsyncRead>(r: &mut R, extensions: &setup::Extensions) -> Result<Self, DecodeError> {
		// Try reading the first byte, returning a special error if the stream naturally ended.
		let typ = match r.read_u8().await {
			Ok(b) => VarInt::decode_byte(b, r).await?,
			Err(e) if e.kind() == io::ErrorKind::UnexpectedEof => return Err(DecodeError::Final),
			Err(e) => return Err(e.into()),
		};

		let size_present = match typ.into_inner() {
			0 => false,
			2 => true,
			_ => return Err(DecodeError::InvalidMessage(typ)),
		};

		let track = VarInt::decode(r).await?;
		let group = VarInt::decode(r).await?;
		let sequence = VarInt::decode(r).await?;
		let priority = VarInt::decode(r).await?.try_into()?;

		let expires = match extensions.object_expires {
			true => match VarInt::decode(r).await?.into_inner() {
				0 => None,
				secs => Some(time::Duration::from_secs(secs)),
			},
			false => None,
		};

		// The presence of the size field depends on the type.
		let size = match size_present {
			true => Some(VarInt::decode(r).await?),
			false => None,
		};

		Ok(Self {
			track,
			group,
			sequence,
			priority,
			expires,
			size,
		})
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W, extensions: &setup::Extensions) -> Result<(), EncodeError> {
		// The kind changes based on the presence of the size.
		let kind = match self.size {
			Some(_) => VarInt::from_u32(2),
			None => VarInt::ZERO,
		};

		kind.encode(w).await?;
		self.track.encode(w).await?;
		self.group.encode(w).await?;
		self.sequence.encode(w).await?;
		VarInt::from_u32(self.priority).encode(w).await?;

		// Round up if there's any decimal points.
		let expires = match self.expires {
			None => 0,
			Some(time::Duration::ZERO) => return Err(EncodeError::InvalidValue), // there's no way of expressing zero currently.
			Some(expires) if expires.subsec_nanos() > 0 => expires.as_secs() + 1,
			Some(expires) => expires.as_secs(),
		};

		if extensions.object_expires {
			VarInt::try_from(expires)?.encode(w).await?;
		}

		if let Some(size) = self.size {
			size.encode(w).await?;
		}

		Ok(())
	}
}
