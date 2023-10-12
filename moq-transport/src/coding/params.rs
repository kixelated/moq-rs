use std::cmp::min;

use crate::VarInt;

use super::{AsyncRead, AsyncWrite, DecodeError, EncodeError};
use tokio::io::AsyncReadExt;

// I hate this parameter encoding so much.
// i hate it i hate it i hate it

// TODO Use #[async_trait] so we can do Param<VarInt> instead.
pub struct ParamInt(pub VarInt);

impl ParamInt {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		// Why do we have a redundant size in front of each VarInt?
		let size = VarInt::decode(r).await?;
		let mut take = r.take(size.into_inner());
		let value = VarInt::decode(&mut take).await?;

		// Like seriously why do I have to check if the VarInt length mismatches.
		if take.limit() != 0 {
			return Err(DecodeError::InvalidSize);
		}

		Ok(Self(value))
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		// Seriously why do I have to compute the size.
		let size = self.0.size();
		VarInt::try_from(size)?.encode(w).await?;

		self.0.encode(w).await?;

		Ok(())
	}
}

pub struct ParamBytes(pub Vec<u8>);

impl ParamBytes {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
		let size = VarInt::decode(r).await?;
		let mut take = r.take(size.into_inner());
		let mut buf = Vec::with_capacity(min(take.limit() as usize, 1024));
		take.read_to_end(&mut buf).await?;

		Ok(Self(buf))
	}

	pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
		let size = VarInt::try_from(self.0.len())?;
		size.encode(w).await?;
		w.write_all(&self.0).await?;

		Ok(())
	}
}

pub struct ParamUnknown {}

impl ParamUnknown {
	pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<(), DecodeError> {
		// Really? Is there no way to advance without reading?
		ParamBytes::decode(r).await?;
		Ok(())
	}
}
