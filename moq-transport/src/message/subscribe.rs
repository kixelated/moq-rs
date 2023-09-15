use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use crate::coding::{AsyncRead, AsyncWrite};

/// Sent by the subscriber to request all future objects for the given track.
///
/// Objects will use the provided ID instead of the full track name, to save bytes.
#[derive(Clone, Debug)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub id: VarInt,

	// The track namespace.
	pub namespace: String,

	// The track name.
	pub name: String,
}

impl Subscribe {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let id = VarInt::decode(r).await?;
		let namespace = decode_string(r).await?;
		let name = decode_string(r).await?;

		Ok(Self { id, namespace, name })
	}
}

impl Subscribe {
	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.id.encode(w).await?;
		encode_string(&self.namespace, w).await?;
		encode_string(&self.name, w).await?;

		Ok(())
	}
}
