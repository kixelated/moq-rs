use crate::coding::{Decode, Duration, Encode, Size, VarInt};
use bytes::{Buf, BufMut};

#[derive(Default)]
pub struct SubscribeOk {
	// NOTE: No full track name because of this proposal: https://github.com/moq-wg/moq-transport/issues/209

	// The ID for this track.
	track_id: VarInt,

	// When non-zero, the subscription will end after this duration has elapsed.
	expires: Duration,
}

impl Decode for SubscribeOk {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r)?;
		let expires = Duration::decode(r)?;

		Ok(Self { track_id, expires })
	}
}

impl Encode for SubscribeOk {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_id.encode(w)?;
		self.expires.encode(w)?;

		Ok(())
	}
}

impl Size for SubscribeOk {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_id.size()? + self.expires.size()?)
	}
}
