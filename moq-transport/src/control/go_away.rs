use crate::coding::{Decode, Encode, Size};
use bytes::{Buf, BufMut};

#[derive(Default)]
pub struct GoAway {
	pub url: String,
}

impl Decode for GoAway {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let url = String::decode(r)?;
		Ok(Self { url })
	}
}

impl Encode for GoAway {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.url.encode(w)
	}
}

impl Size for GoAway {
	fn size(&self) -> anyhow::Result<usize> {
		self.url.size()
	}
}
