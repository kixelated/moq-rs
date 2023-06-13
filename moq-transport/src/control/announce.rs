use crate::coding::{Decode, Encode, Params, Size, VarInt};
use bytes::Bytes;

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

pub struct Announce {
	// The track namespace
	pub track_namespace: String,

	// An authentication token, param 0x02
	pub auth: Option<Bytes>,

	// Parameters that we don't recognize.
	pub unknown: Params,
}

#[async_trait(?Send)]
impl Decode for Announce {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_namespace = String::decode(r).await?;

		let mut auth = None;
		let unknown = Params::new();

		while let Ok(id) = VarInt::decode(r).await {
			let dup = match u64::from(id) {
				2 => auth.replace(Bytes::decode(r).await?).is_some(),
				_ => anyhow::bail!("unknown parameter: {}", id), //unknown.decode_param(r)?,
			};

			anyhow::ensure!(!dup, "duplicate parameter: {}", id)
		}

		Ok(Self {
			track_namespace,
			auth,
			unknown,
		})
	}
}

#[async_trait(?Send)]
impl Encode for Announce {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_namespace.encode(w).await?;

		if let Some(auth) = &self.auth {
			VarInt(2).encode(w).await?;
			auth.encode(w).await?;
		}

		self.unknown.encode(w).await?;

		Ok(())
	}
}

impl Size for Announce {
	fn size(&self) -> anyhow::Result<usize> {
		let mut size = self.track_namespace.size()? + self.unknown.size()?;

		if let Some(auth) = &self.auth {
			size += VarInt(2).size()? + auth.size()?;
		}

		Ok(size)
	}
}
