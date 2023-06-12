use bytes::{BufMut, Bytes};

use super::VarInt;

pub trait Encode: Sized {
	fn encode<B: BufMut>(&self, buf: &mut B) -> anyhow::Result<()>;
}

impl Encode for Bytes {
	fn encode<B: BufMut>(&self, buf: &mut B) -> anyhow::Result<()> {
		VarInt::try_from(self.len())?.encode(buf)?;
		buf.put_slice(self);
		Ok(())
	}
}

impl Encode for Vec<u8> {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		VarInt::try_from(self.len())?.encode(w)?;
		w.put_slice(self);
		Ok(())
	}
}

impl<T: Encode> Encode for Vec<T> {
	fn encode<B: bytes::BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		let len = VarInt::try_from(self.len())?;
		len.encode(w)?;

		for item in self {
			item.encode(w)?;
		}

		Ok(())
	}
}

impl Encode for String {
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		self.as_bytes().to_vec().encode(w)
	}
}
