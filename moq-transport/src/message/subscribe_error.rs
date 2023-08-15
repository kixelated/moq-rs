use crate::coding::{decode_string, encode_string, DecodeError, EncodeError, VarInt};

use webtransport_generic::{RecvStream, SendStream};

#[derive(Debug)]
pub struct SubscribeError {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	pub track_id: VarInt,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

impl SubscribeError {
	pub async fn decode<R: RecvStream>(r: &mut R) -> Result<Self, DecodeError> {
		let track_id = VarInt::decode(r).await?;
		let code = VarInt::decode(r).await?;
		let reason = decode_string(r).await?;

		Ok(Self { track_id, code, reason })
	}
}

impl SubscribeError {
	pub async fn encode<W: SendStream>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_id.encode(w).await?;
		self.code.encode(w).await?;
		encode_string(&self.reason, w).await?;

		Ok(())
	}
}
