use crate::coding::{Decode, Encode, Size, VarInt};

use std::collections::HashMap;

use bytes::Bytes;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Default, Debug)]
pub struct Params(pub HashMap<VarInt, Bytes>);

#[async_trait(?Send)]
impl Decode for Params {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let mut map = Self::new();

		while let Ok(id) = VarInt::decode(r).await {
			map.decode_one(id, r).await?
		}

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
	fn size(&self) -> usize {
		let mut size = 0;

		for (id, value) in &self.0 {
			size += id.size() + value.size();
		}

		size
	}
}

impl Params {
	pub fn new() -> Self {
		Default::default()
	}

	// Decode a single parameter from the buffer and insert it.
	pub async fn decode_one<R: AsyncRead + Unpin>(&mut self, id: VarInt, r: &mut R) -> anyhow::Result<()> {
		let value = Bytes::decode(r).await?;
		let existing = self.0.insert(id, value);
		anyhow::ensure!(existing.is_none(), "duplicate parameter: {}", id);

		Ok(())
	}
}
