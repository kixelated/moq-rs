use std::io;

use crate::coding::{Encode, EncodeError};

use bytes::Buf;

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
		let mut cursor = io::Cursor::new(buf);

		while cursor.has_remaining() {
			let size = self.stream.write_buf(&mut cursor).await?;
			if size == 0 {
				return Err(EncodeError::More(cursor.remaining()).into());
			}
		}

		Ok(())
	}

	pub fn reset(&mut self, code: u32) {
		self.stream.reset(code)
	}

	pub fn id(&self) -> u64 {
		self.stream.id()
	}
}
