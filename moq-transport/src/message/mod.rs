mod announce;
mod announce_ok;
mod announce_reset;
mod announce_stop;
mod go_away;
mod object;
mod subscribe;
mod subscribe_ok;
mod subscribe_reset;
mod subscribe_stop;

pub use announce::*;
pub use announce_ok::*;
pub use announce_reset::*;
pub use announce_stop::*;
pub use go_away::*;
pub use object::*;
pub use subscribe::*;
pub use subscribe_ok::*;
pub use subscribe_reset::*;
pub use subscribe_stop::*;

use crate::coding::{DecodeError, EncodeError, VarInt};

use std::fmt;

use crate::coding::{AsyncRead, AsyncWrite};

// NOTE: This is forked from moq-transport-00.
//   1. SETUP role indicates local support ("I can subscribe"), not remote support ("server must publish")
//   2. SETUP_SERVER is id=2 to disambiguate
//   3. messages do not have a specified length.
//   4. messages are sent over a single bidrectional stream (after SETUP), not unidirectional streams.
//   5. SUBSCRIBE specifies the track_id, not SUBSCRIBE_OK
//   6. optional parameters are written in order, and zero when unset (setup, announce, subscribe)

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		#[derive(Clone)]
		pub enum Message {
			$($name($name)),*
		}

		impl Message {
			pub async fn decode<R: AsyncRead>(r: &mut R) -> Result<Self, DecodeError> {
				let t = VarInt::decode(r).await?;

				match t.into_inner() {
					$($val => {
						let msg = $name::decode(r).await?;
						Ok(Self::$name(msg))
					})*
					_ => Err(DecodeError::InvalidType(t)),
				}
			}

			pub async fn encode<W: AsyncWrite>(&self, w: &mut W) -> Result<(), EncodeError> {
				match self {
					$(Self::$name(ref m) => {
						VarInt::from_u32($val).encode(w).await?;
						m.encode(w).await
					},)*
				}
			}

			pub fn id(&self) -> VarInt {
				match self {
					$(Self::$name(_) => {
						VarInt::from_u32($val)
					},)*
				}
			}

			pub fn name(&self) -> &'static str {
				match self {
					$(Self::$name(_) => {
						stringify!($name)
					},)*
				}
			}
		}

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

// Each message is prefixed with the given VarInt type.
message_types! {
	// NOTE: Object and Setup are in other modules.
	// Object = 0x0
	// SetupClient = 0x1
	// SetupServer = 0x2
	Subscribe = 0x3,
	SubscribeOk = 0x4,
	SubscribeReset = 0x5,
	SubscribeStop = 0x15,
	Announce = 0x6,
	AnnounceOk = 0x7,
	AnnounceStop = 0x8,
	AnnounceReset = 0x18,
	GoAway = 0x10,
}
