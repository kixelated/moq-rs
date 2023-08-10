use std::cmp::min;

use tokio::io::{AsyncReadExt, AsyncWriteExt};
use webtransport_generic::{RecvStream, SendStream};

use crate::VarInt;

use super::{DecodeError, EncodeError};

pub async fn encode_string<W: SendStream>(s: &str, w: &mut W) -> Result<(), EncodeError> {
	let size = VarInt::try_from(s.len())?;
	size.encode(w).await?;
	w.write_all(s.as_ref()).await?;
	Ok(())
}

pub async fn decode_string<R: RecvStream>(r: &mut R) -> Result<String, DecodeError> {
	let size = VarInt::decode(r).await?.into_inner();
	let mut str = String::with_capacity(min(1024, size) as usize);
	r.take(size).read_to_string(&mut str).await?;
	Ok(str)
}
