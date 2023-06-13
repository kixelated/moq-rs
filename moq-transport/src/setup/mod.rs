mod client;
mod object;
mod server;

pub use client::*;
pub use object::*;
pub use server::*;

use crate::coding::{Decode, Encode, Size, VarInt};

use anyhow::Context;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		pub enum Message {
			$($name($name)),*
		}

		#[async_trait(?Send)]
		impl Decode for Message {
			async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
				let t = VarInt::decode(r).await.context("failed to decode type")?;
				let size = VarInt::decode(r).await.context("failed to decode size")?;
				let mut r = r.take(size.into());

				let v = match t.into() {
					$(VarInt($val) => Self::$name($name::decode(&mut r).await.context("failed to decode $name")?),)*
					_ => anyhow::bail!("invalid type: {}", t),
				};

				// Sanity check: make sure we decoded the entire message.
				let mut buf = [0];
				anyhow::ensure!(r.read(&mut buf).await? == 0, "partial decode");

				Ok(v)
			}
		}

		#[async_trait(?Send)]
		impl Encode for Message {
			async fn encode<W: AsyncWrite + Unpin>(&self, w: &mut W) -> anyhow::Result<()> {
				match self {
					$(Self::$name(ref m) => {
						VarInt($val).encode(w).await.context("failed to encode type")?;

						let size = m.size().context("failed to compute size")?;
						let size = VarInt::try_from(size).context("size too large")?;
						size.encode(w).await.context("failed to encode size")?;

						// TODO sanity check: make sure we write exactly size bytes
						m.encode(w).await.context("failed to encode $name")
					},)*
				}
			}
		}

		impl Size for Message {
			fn size(&self) -> anyhow::Result<usize> {
				Ok(match self {
					$(Self::$name(ref m) => {
						let size = m.size()?;
						VarInt($val).size().unwrap() + VarInt::try_from(size)?.size()? + size
					},)*
				})
			}
		}

		// Unwrap the enum into the specified type.
		$(impl TryFrom<Message> for $name {
			type Error = anyhow::Error;

			fn try_from(m: Message) -> Result<Self, Self::Error> {
				match m {
					Message::$name(m) => Ok(m),
					_ => anyhow::bail!("invalid message type"),
				}
			}
		})*
    }
}

// Each message is prefixed with the given VarInt type.
message_types! {
	Object = 0x00,
	Client = 0x01,
	Server = 0x02, // proposal: moq-wg/moq-transport#212
}
