use super::{Close, Reader, Writer};
use crate::Error;

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Close for Stream {
	fn close(&mut self, err: Error) {
		self.writer.close(err.clone());
		self.reader.close(err);
	}
}
