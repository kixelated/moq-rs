use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use bytes::{Buf, BufMut};

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

impl Decode for SubscribeError {
	fn decode<R: Buf>(r: &mut R) -> Result<Self, DecodeError> {
		let track_id = VarInt::decode(r)?;
		let code = VarInt::decode(r)?;
		let reason = String::decode(r)?;

		Ok(Self { track_id, code, reason })
	}
}

impl Encode for SubscribeError {
	fn encode<W: BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.track_id.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;

		Ok(())
	}
}
