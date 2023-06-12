use super::{Decode, Encode, Size, VarInt};
use bytes::{Buf, BufMut};

use std::time;

#[derive(Default)]
pub struct Duration(pub time::Duration);

impl Encode for Duration {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		let ms = self.0.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.encode(w)
	}
}

impl Decode for Duration {
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		let ms = VarInt::decode(r)?;
		let ms = ms.into();
		Ok(Self(time::Duration::from_millis(ms)))
	}
}

impl Size for Duration {
	fn size(&self) -> anyhow::Result<usize> {
		let ms = self.0.as_millis();
		let ms = VarInt::try_from(ms)?;
		ms.size()
	}
}

impl From<Duration> for time::Duration {
	fn from(d: Duration) -> Self {
		d.0
	}
}

impl From<time::Duration> for Duration {
	fn from(d: time::Duration) -> Self {
		Self(d)
	}
}
