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
use std::fmt;
use tokio::io::{AsyncRead, AsyncWrite};

use anyhow::Context;

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		pub enum Message {
			$($name($name)),*
		}

		#[async_trait]
		impl Decode for Message {
			async fn decode<R: AsyncRead + Unpin + Send>(r: &mut R) -> anyhow::Result<Self> {
				let t = u64::decode(r).await.context("failed to decode type")?;

				Ok(match u64::from(t) {
					$($val => {
						let msg = $name::decode(r).await.context(concat!("failed to decode ", stringify!($name)))?;
						Self::$name(msg)
					})*
					_ => anyhow::bail!("invalid type: {}", t),
				})
			}
		}

		#[async_trait]
		impl Encode for Message {
			async fn encode<W: AsyncWrite + Unpin + Send>(&self, w: &mut W) -> anyhow::Result<()> {
				match self {
					$(Self::$name(ref m) => {
						let id: u64 = $val; // tell the compiler this is a u64
						id.encode(w).await.context("failed to encode type")?;
						m.encode(w).await.context("failed to encode message")
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

		$(impl From<$name> for Message {
			fn from(m: $name) -> Self {
				Message::$name(m)
			}
		})*

		impl fmt::Debug for Message {
			// Delegate to the message formatter
			fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
				match self {
					$(Self::$name(ref m) => m.fmt(f),)*
				}
			}
		}
    }
}

// NOTE: These messages are forked from moq-transport-00.
//   1. subscribe specifies the track_id, not subscribe_ok
//   2. messages lack a specified length
//   3. optional parameters are not supported (announce, subscribe)
//   4. not allowed on undirectional streams; only after SETUP on the bidirectional stream

// Each message is prefixed with the given VarInt type.
message_types! {
	// NOTE: Object and Setup are in other modules.
	// Object = 0x0
	// Setup  = 0x1
	Subscribe = 0x3,
	SubscribeOk = 0x4,
	SubscribeError = 0x5,
	Announce = 0x6,
	AnnounceOk = 0x7,
	AnnounceError = 0x8,
	GoAway = 0x10,
}
