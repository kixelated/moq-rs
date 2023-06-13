use crate::coding::{Decode, Encode, Size, VarInt};

use std::collections::HashMap;

use bytes::Bytes;
/*
#[derive(Default)]
pub struct Param<const I: u32, T>(pub Option<T>)
where
	T: Encode + Decode + Size;

impl<const I: u32, T> Encode for Param<I, T>
where
	T: Encode + Decode + Size,
{
	fn encode<B: BufMut>(&self, w: &mut B) -> anyhow::Result<()> {
		if let Some(value) = &self.0 {
			Self::ID.encode(w)?;
			value.encode(w)?;
		}

		Ok(())
	}
}

impl<const I: u32, T> Decode for Param<I, T>
where
	T: Encode + Decode + Size,
{
	fn decode<B: Buf>(r: &mut B) -> anyhow::Result<Self> {
		// NOTE: assumes the ID has already been decoded.
		let value = T::decode(r)?;
		Ok(Self(Some(value)))
	}
}

impl<const I: u32, T> Size for Param<I, T>
where
	T: Encode + Decode + Size,
{
	fn size(&self) -> anyhow::Result<usize> {
		if let Some(value) = &self.0 {
			Ok(Self::ID.size()? + value.size()?)
		} else {
			Ok(0)
		}
	}
}

impl<const I: u32, T> Param<I, T>
where
	T: Encode + Decode + Size,
{
	const ID: VarInt = VarInt::from_u32(I);

	pub fn new() -> Self {
		Self(None)
	}
}
*/

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Default)]
pub struct Params(pub HashMap<VarInt, Bytes>);

#[async_trait(?Send)]
impl Decode for Params {
	async fn decode<R: AsyncRead + Unpin>(_r: &mut R) -> anyhow::Result<Self> {
		let map = Self::new();

		/*
		loop {
			let id = VarInt::decode(r).await?;
		}
		while r.read(&[]).await? > 0 {
			map.decode_param(r).await?;
		}
		*/

		Ok(map)
	}
}

#[async_trait(?Send)]
impl Encode for Params {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		for (id, value) in &self.0 {
			id.encode(w).await?;
			value.encode(w).await?;
		}

		Ok(())
	}
}

impl Size for Params {
	fn size(&self) -> anyhow::Result<usize> {
		let mut size = 0;

		for (id, value) in &self.0 {
			size += id.size()? + value.size()?;
		}

		Ok(size)
	}
}

impl Params {
	pub fn new() -> Self {
		Default::default()
	}

	/*
	// Decode a single parameter from the buffer.
	pub fn decode_param<B: Buf>(&mut self, r: &mut B) -> anyhow::Result<()> {
		let id = VarInt::decode(r)?;
		let value = Bytes::decode(r)?;

		self.0.insert(id, value);

		Ok(())
	}
	*/
}
