use crate::coding::{Decode, Encode, Size};
use bytes::{Buf, BufMut};

#[derive(Default)]
pub struct AnnounceOk {
	// Echo back the namespace that was announced.
	// TODO Propose using an ID to save bytes.
	pub track_namespace: String,
}

impl Decode for AnnounceOk {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r)?;
		Ok(Self { track_namespace })
	}
}

impl Encode for AnnounceOk {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.track_namespace.encode(w)
	}
}

impl Size for AnnounceOk {
	fn size(&self) -> anyhow::Result<usize> {
		self.track_namespace.size()
	}
}
