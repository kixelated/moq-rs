use std::{cmp, io};

use bytes::Buf;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::coding::Decode;

use super::DecodeError;

pub struct Reader<S: AsyncRead + Unpin> {
	stream: S,
	buffer: bytes::BytesMut,
}

impl<S: AsyncRead + Unpin> Reader<S> {
	pub fn new(stream: S) -> Self {
		Self {
			stream,
			buffer: Default::default(),
		}
	}

	pub async fn decode<T: Decode>(&mut self) -> Result<T, DecodeError> {
		loop {
			let mut cursor = io::Cursor::new(&self.buffer);

			// Try to decode with the current buffer.
			let mut remain = match T::decode(&mut cursor) {
				Ok(msg) => {
					self.buffer.advance(cursor.position() as usize);
					return Ok(msg);
				}
				Err(DecodeError::More(remain)) => remain,
				Err(err) => return Err(err),
			};

			// Read in more data until we reach the requested amount.
			// We always read at least once to avoid an infinite loop if some dingus puts remain=0
			loop {
				let size = self.stream.read_buf(&mut self.buffer).await?;
				remain = remain.saturating_sub(size);
				if remain == 0 {
					break;
				}
			}
		}
	}

	pub async fn read(&mut self, max_size: usize) -> Result<Option<bytes::Bytes>, io::Error> {
		if self.buffer.is_empty() {
			// TODO avoid making a copy by using Quinn's read_chunk
			let size = self.stream.read_buf(&mut self.buffer).await?;
			if size == 0 {
				return Ok(None);
			}
		}

		let size = cmp::min(self.buffer.len(), max_size);
		Ok(Some(self.buffer.split_to(size).freeze()))
	}

	pub async fn done(&mut self) -> Result<bool, io::Error> {
		Ok(self.buffer.is_empty() && self.stream.read_buf(&mut self.buffer).await? == 0)
	}

	pub fn into_inner(self) -> (bytes::BytesMut, S) {
		(self.buffer, self.stream)
	}
}
