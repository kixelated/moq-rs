use crate::coding::{Decode, Encode, Size, VarInt};
use bytes::{Buf, BufMut};

#[derive(Default)]
pub struct SubscribeError {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	track_id: VarInt,

	// An error code.
	code: VarInt,

	// An optional, human-readable reason.
	reason: String,
}

impl Decode for SubscribeError {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r)?;
		let code = VarInt::decode(r)?;
		let reason = String::decode(r)?;

		Ok(Self { track_id, code, reason })
	}
}

impl Encode for SubscribeError {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_id.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;

		Ok(())
	}
}

impl Size for SubscribeError {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()? + self.code.size()? + self.reason.size()?)
	}
}
