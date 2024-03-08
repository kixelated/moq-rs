use crate::coding::{AsyncRead, AsyncWrite, Decode, DecodeError, Encode, EncodeError, VarInt};
use std::fmt;

use super::{
	Announce, AnnounceError, AnnounceOk, GoAway, Subscribe, SubscribeDone, SubscribeError, SubscribeOk, Unannounce,
	Unsubscribe,
};

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		/// All supported message types.
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
					_ => Err(DecodeError::InvalidMessage(t)),
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
	// ObjectUnbounded = 0x2
	// SetupClient = 0x40
	// SetupServer = 0x41

	// SUBSCRIBE family, sent by subscriber
	Subscribe = 0x3,
	Unsubscribe = 0xa,

	// SUBSCRIBE family, sent by publisher
	SubscribeOk = 0x4,
	SubscribeError = 0x5,
	SubscribeDone = 0xb,

	// ANNOUNCE family, sent by publisher
	Announce = 0x6,
	Unannounce = 0x9,

	// ANNOUNCE family, sent by subscriber
	AnnounceOk = 0x7,
	AnnounceError = 0x8,

	// Misc
	GoAway = 0x10,
}
