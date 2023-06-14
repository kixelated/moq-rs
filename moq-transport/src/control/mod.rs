mod announce;
mod announce_error;
mod announce_ok;
mod go_away;
mod stream;
mod subscribe;
mod subscribe_error;
mod subscribe_ok;

pub use announce::*;
pub use announce_error::*;
pub use announce_ok::*;
pub use go_away::*;
pub use stream::*;
pub use subscribe::*;
pub use subscribe_error::*;
pub use subscribe_ok::*;

use crate::coding::{Decode, Encode};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use anyhow::Context;

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		#[derive(Debug)]
		pub enum Message {
			$($name($name)),*
		}

		#[async_trait]
		impl Decode for Message {
			async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
				let t = u64::decode(r).await.context("failed to decode type")?;
				let size = u64::decode(r).await.context("failed to decode size")?;
				let mut r = r.take(size);

				let v = match u64::from(t) {
					$($val => Self::$name($name::decode(&mut r).await.context("failed to decode $name")?),)*
					_ => anyhow::bail!("invalid type: {}", t),
				};

				// Sanity check: make sure we decoded the entire message.
				let mut buf = [0];
				anyhow::ensure!(r.read(&mut buf).await? == 0, "partial decode");

				Ok(v)
			}
		}

		#[async_trait]
		impl Encode for Message {
			async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {

				match self {
					$(Self::$name(ref m) => {
						let id: u64 = $val; // tell the compiler this is a u64
						id.encode(w).await.context("failed to encode type")?;

						let mut buf = Vec::new();
						m.encode(&mut buf).await.context("failed to encode message")?;
						buf.len().encode(w).await.context("failed to encode size")?;

						w.write_all(&buf).await.context("failed to write message")?;
					},)*
				}

				Ok(())
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

		$(impl From<$name> for Message {
			fn from(m: $name) -> Self {
				Message::$name(m)
			}
		})*
    }
}

// Each message is prefixed with the given VarInt type.
message_types! {
	// NOTE: Object and Setup are in the setup module.
	// see issues: moq-wg/moq-transport#212 and moq-wg/moq-transport#138
	Subscribe = 0x3,
	SubscribeOk = 0x4,
	SubscribeError = 0x5,
	Announce = 0x6,
	AnnounceOk = 0x7,
	AnnounceError = 0x8,
	GoAway = 0x10,
}
