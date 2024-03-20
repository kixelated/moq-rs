use std::cmp::min;

use crate::coding::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use super::{Decode, DecodeError, Encode, EncodeError};

#[async_trait::async_trait]
impl Encode for String {
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		self.len().encode(w).await?;
		w.write_all(self.as_ref()).await.map_err(|_| EncodeError::IoError)?;
		Ok(())
	}
}

#[async_trait::async_trait]
impl Decode for String {
	/// Decode a string with a varint length prefix.
	async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let size = usize::decode(r).await?;
		let mut str = String::with_capacity(min(1024, size));
		r.take(size as u64)
			.read_to_string(&mut str)
			.await
			.map_err(|_| DecodeError::IoError)?;
		Ok(str)
	}
}
