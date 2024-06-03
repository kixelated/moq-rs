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
				if !self.stream.read_buf(&mut self.buffer).await? {
					return Err(DecodeError::More(required - self.buffer.len()).into());
				};

				if self.buffer.len() >= required {
					break;
				}
			}
		}
	}

	// Decode optional messages at the end of a stream
	// The weird order of Option<Result is for tokio::select!
	pub async fn decode_maybe<T: Decode>(&mut self) -> Option<Result<T, SessionError>> {
		match self.closed().await {
			Ok(()) => None,
			Err(SessionError::Decode(DecodeError::ExpectedData)) => Some(self.decode().await),
			Err(e) => Some(Err(e)),
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

	pub fn stop(&mut self, code: u32) {
		self.stream.stop(code)
	}

	// Wait until the stream is closed, ensuring there are no additional bytes
	pub async fn closed(&mut self) -> Result<(), SessionError> {
		if self.buffer.is_empty() {
			if !self.stream.read_buf(&mut self.buffer).await? {
				return Ok(());
			}
		}

		Err(SessionError::Decode(DecodeError::ExpectedEnd))
	}
}
