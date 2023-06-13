use crate::coding::{Decode, Encode, Size, VarInt};
use bytes::{Buf, BufMut};

#[derive(Default)]
pub struct AnnounceError {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,

	// An error code.
	pub code: VarInt,

	// An optional, human-readable reason.
	pub reason: String,
}

impl Decode for AnnounceError {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r)?;
		let code = VarInt::decode(r)?;
		let reason = String::decode(r)?;

		Ok(Self {
			track_namespace,
			code,
			reason,
		})
	}
}

impl Encode for AnnounceError {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_namespace.encode(w)?;
		self.code.encode(w)?;
		self.reason.encode(w)?;

		Ok(())
	}
}

impl Size for AnnounceError {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(self.track_namespace.size()? + self.code.size()? + self.reason.size()?)
	}
}
