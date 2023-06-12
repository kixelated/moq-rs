use super::{Decode, Encode, VarInt};
use bytes::buf::UninitSlice;
use bytes::{Buf, BufMut, Bytes};

pub trait Size {
	fn size(&self) -> anyhow::Result<usize>;
}

impl Size for Bytes {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(VarInt::try_from(self.len())?.size()? + self.len())
	}
}

impl Size for Vec<u8> {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(VarInt::try_from(self.len())?.size()? + self.len())
	}
}

impl<T: Size> Size for Vec<T> {
	fn size(&self) -> anyhow::Result<usize> {
		let mut size = VarInt::try_from(self.len())?.size()?;
		for t in self.iter() {
			size += t.size()?;
		}
		Ok(size)
	}
}

impl Size for String {
	fn size(&self) -> anyhow::Result<usize> {
		Ok(VarInt::try_from(self.len())?.size()? + self.len())
	}
}

// Helpers to make encoding a size + value easier.
pub struct WithSize;

impl WithSize {
	pub fn encode<B: BufMut, T: Size + Encode>(w: &mut B, t: &T) -> anyhow::Result<()> {
		let size = t.size()?;
		VarInt::try_from(size)?.encode(w)?;

		// Ensure that we're encoding the correct number of bytes.
		// TODO remove this when we're confident.
		let mut w = BufMutSize { buf: w, size: 0 };
		t.encode(&mut w)?;

		anyhow::ensure!(w.size == size, "wrong size reported");
		Ok(())
	}

	pub fn decode<B: Buf, T: Size + Decode>(r: &mut B) -> anyhow::Result<T> {
		let size = VarInt::decode(r)?;
		let mut r = r.take(size.into());
		let t = T::decode(&mut r)?;
		anyhow::ensure!(!r.has_remaining(), "short decode");
		Ok(t)
	}

	pub fn size<T: Size>(t: &T) -> anyhow::Result<usize> {
		let size = t.size()?;
		Ok(VarInt::try_from(size)?.size()? + size)
	}
}

// Wrapper to counts the number of bytes written to a buffer.
// TODO remove once we're confident in message encoding.
struct BufMutSize<'a, B: BufMut> {
	buf: &'a mut B,
	size: usize,
}

unsafe impl<B> BufMut for BufMutSize<'_, B>
where
	B: BufMut,
{
	fn remaining_mut(&self) -> usize {
		self.buf.remaining_mut()
	}

	unsafe fn advance_mut(&mut self, cnt: usize) {
		self.buf.advance_mut(cnt);
		self.size += cnt;
	}

	fn chunk_mut(&mut self) -> &mut UninitSlice {
		self.buf.chunk_mut()
	}
}
