use super::{Close, Reader, Writer};

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Close for Stream {
	fn close(&mut self, code: u32) {
		self.writer.close(code);
		self.reader.close(code);
	}
}
