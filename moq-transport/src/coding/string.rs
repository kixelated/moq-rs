use std::cmp::min;

use crate::coding::{AsyncRead, AsyncWrite};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::VarInt;

use super::{DecodeError, EncodeError};

/// Encode a string with a varint length prefix.
pub async fn encode_string<W: AsyncWrite>(s: &str, w: &mut W) -> Result<(), EncodeError> {
	let size = VarInt::try_from(s.len())?;
	size.encode(w).await?;
	w.write_all(s.as_ref()).await?;
	Ok(())
}

/// Decode a string with a varint length prefix.
pub async fn decode_string<R: AsyncRead>(r: &mut R) -> Result<String, DecodeError> {
	let size = VarInt::decode(r).await?.into_inner();
	let mut str = String::with_capacity(min(1024, size) as usize);
	r.take(size).read_to_string(&mut str).await?;
	Ok(str)
}
