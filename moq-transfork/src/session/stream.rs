use super::{Close, Reader, SessionError, Writer};

pub struct Stream {
	pub writer: Writer,
	pub reader: Reader,
}

impl Close for Stream {
	fn close(&mut self, err: SessionError) {
		self.writer.close(err.clone());
		self.reader.close(err);
	}
}
