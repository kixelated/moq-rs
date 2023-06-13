mod client;
mod role;
mod server;
mod version;

pub use client::*;
pub use role::*;
pub use server::*;
pub use version::*;

use crate::coding::{Decode, Encode};

use anyhow::Context;
use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		#[derive(Debug)]
		pub enum Message {
			$($name($name)),*
		}

		#[async_trait(?Send)]
		impl Decode for Message {
			async fn decode<R: AsyncRead + Unpin>(r: &mut R) -> anyhow::Result<Self> {
				let t = u64::decode(r).await.context("failed to decode type")?;
				let size = u64::decode(r).await.context("failed to decode size")?;
				let mut r = r.take(size);

				let v = match t {
					$($val => Self::$name($name::decode(&mut r).await.context("failed to decode $name")?),)*
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
						let id: u64 = $val; // tell the compiler this is a u64
						id.encode(w).await.context("failed to encode type")?;

						let mut buf = Vec::new();
						m.encode(&mut buf).await.context("failed to encode message")?;
						buf.len().encode(w).await.context("failed to encode size")?;

						w.write_all(&buf).await.context("failed to write message")
					},)*
				}
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
	Client = 0x1,
	Server = 0x2, // proposal: moq-wg/moq-transport#212
}
