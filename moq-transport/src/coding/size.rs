use super::VarInt;
use bytes::Bytes;

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

/*
pub fn size<T: Encode>(v: T) -> anyhow::Result<usize> {
	let mut sizer = Sizer::new();
	v.encode(&mut sizer)?;
	Ok(sizer.count)
}

#[derive(Default)]
struct Sizer {
	scratch: [u8; 32],
	pub count: usize,
}

unsafe impl BufMut for Sizer {
	fn remaining_mut(&self) -> usize {
		self.scratch.len()
	}

	unsafe fn advance_mut(&mut self, count: usize) {
		self.count += count;
	}

	fn chunk_mut(&mut self) -> &mut UninitSlice {
		unsafe { UninitSlice::from_raw_parts_mut(self.scratch.as_mut_ptr(), self.count) }
	}
}

impl Sizer {
	pub fn new() -> Self {
		Default::default()
	}
}
*/
