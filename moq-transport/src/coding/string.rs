use std::cmp::min;

use crate::coding::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::VarInt;

use super::{Decode, DecodeError, Encode, EncodeError};

#[async_trait::async_trait]
impl Encode for String {
	async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		let size = VarInt::try_from(self.len())?;
		size.encode(w).await?;
		w.write_all(self.as_ref()).await?;
		Ok(())
	}
}

#[async_trait::async_trait]
impl Decode for String {
	/// Decode a string with a varint length prefix.
	async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let size = VarInt::decode(r).await?.into_inner();
		let mut str = String::with_capacity(min(1024, size) as usize);
		r.take(size).read_to_string(&mut str).await?;
		Ok(str)
	}
}
