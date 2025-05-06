use std::fmt;

use crate::{coding::*, message, Error};

// A wrapper around a web_transport::SendStream that will reset on Drop
pub(super) struct Writer {
	stream: Option<web_transport::SendStream>,
	buffer: bytes::BytesMut,
}

impl Writer {
	pub fn new(stream: web_transport::SendStream) -> Self {
		Self {
			stream: Some(stream),
			buffer: Default::default(),
		}
	}

	pub async fn open(session: &mut web_transport::Session, typ: message::DataType) -> Result<Self, Error> {
		let send = session.open_uni().await?;

		let mut writer = Self::new(send);
		writer.encode(&typ).await?;

		Ok(writer)
	}

	pub async fn encode<T: Encode + fmt::Debug>(&mut self, msg: &T) -> Result<(), Error> {
		self.buffer.clear();
		msg.encode(&mut self.buffer);

		while !self.buffer.is_empty() {
			self.stream.as_mut().unwrap().write_buf(&mut self.buffer).await?;
		}

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
		self.stream.as_mut().unwrap().write(buf).await?; // convert the error type
		Ok(())
	}

	pub fn set_priority(&mut self, priority: i32) {
		self.stream.as_mut().unwrap().set_priority(priority);
	}

	// A clean termination of the stream
	pub fn close(mut self) {
		self.stream.take();
	}
}

impl Drop for Writer {
	fn drop(&mut self) {
		if let Some(mut stream) = self.stream.take() {
			stream.reset(Error::Cancel.to_code());
		}
	}
}
