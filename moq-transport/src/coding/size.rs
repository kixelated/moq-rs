use super::VarInt;
use bytes::Bytes;

pub trait Size {
	fn size(&self) -> usize;
}

impl Size for Bytes {
	fn size(&self) -> usize {
		VarInt::try_from(self.len()).unwrap().size() + self.len()
	}
}

impl Size for Vec<u8> {
	fn size(&self) -> usize {
		VarInt::try_from(self.len()).unwrap().size() + self.len()
	}
}

impl<T: Size> Size for Vec<T> {
	fn size(&self) -> usize {
		let mut size = VarInt::try_from(self.len()).unwrap().size();
		for t in self.iter() {
			size += t.size();
		}
		size
	}
}

impl Size for String {
	fn size(&self) -> usize {
		VarInt::try_from(self.len()).unwrap().size() + self.len()
	}
}
