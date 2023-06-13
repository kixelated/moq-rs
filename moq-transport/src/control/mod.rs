mod announce;
mod announce_error;
mod announce_ok;
mod go_away;
mod subscribe;
mod subscribe_error;
mod subscribe_ok;

pub use announce::*;
pub use announce_error::*;
pub use announce_ok::*;
pub use go_away::*;
pub use subscribe::*;
pub use subscribe_error::*;
pub use subscribe_ok::*;

use crate::coding::{Decode, Encode, Size, VarInt};

use async_trait::async_trait;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};

use anyhow::Context;

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
	// NOTE: Object and Setup are in the setup module.
	// see issues: moq-wg/moq-transport#212 and moq-wg/moq-transport#138
	Subscribe = 0x03,
	SubscribeOk = 0x04,
	SubscribeError = 0x05,
	Announce = 0x06,
	AnnounceOk = 0x07,
	AnnounceError = 0x08,
	GoAway = 0x10,
}
