use std::{fmt, ops};

use crate::coding::{Encode, EncodeError};

#[derive(thiserror::Error, Debug, Clone)]
pub enum WriteError {
	#[error("encode error: {0}")]
	Encode(#[from] EncodeError),

	#[error("webtransport error: {0}")]
	Transport(#[from] web_transport::WriteError),
}

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

	pub async fn encode<T: Encode>(&mut self, msg: &T) -> Result<(), WriteError> {
		self.buffer.clear();
		msg.encode(&mut self.buffer)?;

		while !self.buffer.is_empty() {
			self.stream.write_buf(&mut self.buffer).await?;
		}

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), WriteError> {
		self.stream.write(buf).await?; // convert the error type
		Ok(())
	}

	pub fn reset(&mut self, code: u32) {
		self.stream.reset(code);
	}
}

impl ops::Deref for Writer {
	type Target = web_transport::StreamInfo;

	fn deref(&self) -> &Self::Target {
		&self.stream.info
	}
}
