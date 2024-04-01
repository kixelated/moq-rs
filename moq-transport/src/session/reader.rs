use std::{cmp, io};

use bytes::{Buf, Bytes, BytesMut};

use crate::coding::{Decode, DecodeError};

use super::SessionError;

pub struct Reader<S: webtransport_generic::RecvStream> {
	stream: S,
	buffer: BytesMut,
}

impl<S: webtransport_generic::RecvStream> Reader<S> {
	pub fn new(stream: S) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn decode<T: Decode>(&mut self) -> Result<T, SessionError> {
		loop {
			let mut cursor = io::Cursor::new(&self.buffer);

			// Try to decode with the current buffer.
			let mut remain = match T::decode(&mut cursor) {
				Ok(msg) => {
					self.buffer.advance(cursor.position() as usize);
					return Ok(msg);
				}
				Err(DecodeError::More(remain)) => remain, // Try again with more data
				Err(err) => return Err(err.into()),
			};

			// Read in more data until we reach the requested amount.
			// We always read at least once to avoid an infinite loop if some dingus puts remain=0
			loop {
				let size = self
					.stream
					.read_buf(&mut self.buffer)
					.await
					.map_err(SessionError::from_read)?;
				if size == 0 {
					return Err(DecodeError::More(remain).into());
				}

				remain = remain.saturating_sub(size);
				if remain == 0 {
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

		let chunk = match self.stream.read_chunk().await.map_err(SessionError::from_read)? {
			Some(chunk) if chunk.len() <= max => Some(chunk),
			Some(mut chunk) => {
				// The chunk is too big; add the tail to the buffer for next read.
				self.buffer.extend_from_slice(&chunk.split_off(max));
				Some(chunk)
			}
			None => None,
		};

		Ok(chunk)
	}

	pub async fn done(&mut self) -> Result<bool, SessionError> {
		if !self.buffer.is_empty() {
			return Ok(false);
		}

		let size = self
			.stream
			.read_buf(&mut self.buffer)
			.await
			.map_err(SessionError::from_read)?;
		Ok(size == 0)
	}
}
