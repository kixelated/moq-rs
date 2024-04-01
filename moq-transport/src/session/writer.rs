use std::io;

use crate::coding::{Encode, EncodeError};

use super::SessionError;
use bytes::Buf;

pub struct Writer<S: webtransport_generic::SendStream> {
	stream: S,
	buffer: bytes::BytesMut,
}

impl<S: webtransport_generic::SendStream> Writer<S> {
	pub fn new(stream: S) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn encode<T: Encode>(&mut self, msg: &T) -> Result<(), SessionError> {
		self.buffer.clear();
		msg.encode(&mut self.buffer)?;

		while !self.buffer.is_empty() {
			self.stream
				.write_buf(&mut self.buffer)
				.await
				.map_err(SessionError::from_write)?;
		}

		Ok(())
	}

	pub async fn write(&mut self, buf: &[u8]) -> Result<(), SessionError> {
		let mut cursor = io::Cursor::new(buf);

		while cursor.has_remaining() {
			let size = self
				.stream
				.write_buf(&mut cursor)
				.await
				.map_err(SessionError::from_write)?;
			if size == 0 {
				return Err(EncodeError::More(cursor.remaining()).into());
			}
		}

		Ok(())
	}
}
