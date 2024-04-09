use std::{cmp, io};

use bytes::{Buf, Bytes, BytesMut};

use crate::coding::{Decode, DecodeError};

use super::SessionError;

pub struct Reader {
	stream: web_transport::RecvStream,
	buffer: BytesMut,
}

impl Reader {
	pub fn new(stream: web_transport::RecvStream) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn decode<T: Decode>(&mut self) -> Result<T, SessionError> {
		loop {
			let mut cursor = io::Cursor::new(&self.buffer);

			// Try to decode with the current buffer.
			let required = match T::decode(&mut cursor) {
				Ok(msg) => {
					self.buffer.advance(cursor.position() as usize);
					return Ok(msg);
				}
				Err(DecodeError::More(required)) => self.buffer.len() + required, // Try again with more data
				Err(err) => return Err(err.into()),
			};

			// Read in more data until we reach the requested amount.
			// We always read at least once to avoid an infinite loop if some dingus puts remain=0
			loop {
				if !self.buffer.is_empty() {
					log::debug!(
						"not enough data to decode, reading more: required={} buffer={:?}",
						required,
						self.buffer
					);
				}

				if !self.stream.read_buf(&mut self.buffer).await? {
					return Err(DecodeError::More(required - self.buffer.len()).into());
				};

				if self.buffer.len() >= required {
					break;
				}
			}
		}
	}

	pub async fn read_chunk(&mut self, max: usize) -> Result<Option<Bytes>, SessionError> {
		if !self.buffer.is_empty() {
			let size = cmp::min(max, self.buffer.len());
			let data = self.buffer.split_to(size).freeze();
			return Ok(Some(data));
		}

		Ok(self.stream.read_chunk(max).await?)
	}

	pub async fn done(&mut self) -> Result<bool, SessionError> {
		if !self.buffer.is_empty() {
			return Ok(false);
		}

		Ok(!self.stream.read_buf(&mut self.buffer).await?)
	}
}
