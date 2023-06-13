use crate::coding::{Decode, Encode, Params, Size, VarInt};
use bytes::Bytes;

use anyhow::Context;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncWrite};

#[derive(Debug)]
pub struct Subscribe {
	// An ID we choose so we can map to the track_name.
	// Proposal: https://github.com/moq-wg/moq-transport/issues/209
	pub track_id: VarInt,

	// The track namespace + track name.
	pub track_name: String,

	// The group sequence number, param 0x00
	pub group_sequence: Option<VarInt>,

	// The object sequence number, param 0x01
	pub object_sequence: Option<VarInt>,

	// An authentication token, param 0x02
	pub auth: Option<Bytes>,

	// Parameters that we don't recognize.
	pub unknown: Params,
}

#[async_trait(?Send)]
impl Decode for Subscribe {
	async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
		let track_id = VarInt::decode(r).await?;
		let track_name = String::decode(r).await?;

		let mut group_sequence = None;
		let mut object_sequence = None;
		let mut auth = None;
		let mut unknown = Params::new();

		while let Ok(id) = VarInt::decode(r).await {
			match id {
				VarInt(0x0) => {
					let v = VarInt::decode(r).await.context("failed to decode group sequence")?;
					anyhow::ensure!(group_sequence.replace(v).is_none(), "duplicate group sequence");
				}
				VarInt(0x1) => {
					let v = VarInt::decode(r).await.context("failed to decode object sequence")?;
					anyhow::ensure!(object_sequence.replace(v).is_none(), "duplicate object sequence");
				}
				VarInt(0x2) => {
					let v = Bytes::decode(r).await.context("failed to decode auth")?;
					anyhow::ensure!(auth.replace(v).is_none(), "duplicate auth");
				}
				_ => {
					unknown
						.decode_one(id, r)
						.await
						.context("failed to decode unknown param")?;
				}
			};
		}

		Ok(Self {
			track_id,
			track_name,
			group_sequence,
			object_sequence,
			auth,
			unknown,
		})
	}
}

#[async_trait(?Send)]
impl Encode for Subscribe {
	async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
		self.track_id.encode(w).await?;
		self.track_name.encode(w).await?;

		// TODO this is ugly, figure out how to avoid this duplication.
		if let Some(group_sequence) = &self.group_sequence {
			VarInt(0).encode(w).await?;
			group_sequence.encode(w).await?;
		}

		if let Some(object_sequence) = &self.object_sequence {
			VarInt(1).encode(w).await?;
			object_sequence.encode(w).await?;
		}

		if let Some(auth) = &self.auth {
			VarInt(2).encode(w).await?;
			auth.encode(w).await?;
		}

		self.unknown.encode(w).await?;

		Ok(())
	}
}

impl Size for Subscribe {
	fn size(&self) -> anyhow::Result<usize> {
		let mut size = self.track_id.size()? + self.track_name.size()? + self.unknown.size()?;

		if let Some(group_sequence) = &self.group_sequence {
			size += VarInt(0).size()? + group_sequence.size()?;
		}

		if let Some(object_sequence) = &self.object_sequence {
			size += VarInt(1).size()? + object_sequence.size()?;
		}

		if let Some(auth) = &self.auth {
			size += VarInt(2).size()? + auth.size()?;
		}

		Ok(size)
	}
}
