use std::fmt;

use crate::coding::*;
use crate::util::Close;
use crate::Error;

pub struct Writer {
	stream: web_transport::SendStream,
	buffer: bytes::BytesMut,
}

impl Writer {
	pub fn new(stream: web_transport::SendStream) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn encode<T: Encode + fmt::Debug>(&mut self, msg: &T) -> Result<(), Error> {
		self.buffer.clear();
		msg.encode(&mut self.buffer);

		while !self.buffer.is_empty() {
			self.stream.write_buf(&mut self.buffer).await?;
		}

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
		self.stream.write(buf).await?; // convert the error type
		Ok(())
	}
}

impl Close for Writer {
	fn close(&mut self, err: Error) {
		self.stream.reset(err.to_code());
	}
}
