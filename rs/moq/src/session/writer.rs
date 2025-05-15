use std::fmt;

use crate::{coding::*, message, Error};

// A wrapper around a web_transport::SendStream that will reset on Drop
pub(super) struct Writer {
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
			self.stream.write_buf(&mut self.buffer).await?;
		}

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), Error> {
		self.stream.write(buf).await?; // convert the error type
		Ok(())
	}

	pub fn set_priority(&mut self, priority: i32) {
		self.stream.set_priority(priority);
	}

	/// A clean termination of the stream, waiting for the peer to close.
	pub async fn finish(&mut self) -> Result<(), Error> {
		self.stream.finish()?;
		self.stream.closed().await?; // TODO Return any error code?
		Ok(())
	}

	pub fn abort(&mut self, err: &Error) {
		self.stream.reset(err.to_code());
	}

	pub async fn closed(&mut self) -> Result<(), Error> {
		self.stream.closed().await?;
		Ok(())
	}
}

impl Drop for Writer {
	fn drop(&mut self) {
		// Unlike the Quinn default, we abort the stream on drop.
		self.stream.reset(Error::Cancel.to_code());
	}
}
