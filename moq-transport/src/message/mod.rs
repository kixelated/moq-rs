//! Low-level message sent over the wire, as defined in the specification.
//!
//! All of these messages are sent over a bidirectional QUIC stream.
//! This introduces some head-of-line blocking but preserves ordering.
//! The only exception are OBJECT "messages", which are sent over dedicated QUIC streams.
//!
//! Messages sent by the publisher:
//! - [Announce]
//! - [Unannounce]
//! - [SubscribeOk]
//! - [SubscribeError]
//! - [SubscribeReset]
//! - [Object]
//!
//! Messages sent by the subscriber:
//! - [Subscribe]
//! - [Unsubscribe]
//! - [AnnounceOk]
//! - [AnnounceError]
//!
//! Example flow:
//! ```test
//!  -> ANNOUNCE        namespace="foo"
//!  <- ANNOUNCE_OK     namespace="foo"
//!  <- SUBSCRIBE       id=0 namespace="foo" name="bar"
//!  -> SUBSCRIBE_OK    id=0
//!  -> OBJECT          id=0 sequence=69 priority=4 expires=30
//!  -> OBJECT          id=0 sequence=70 priority=4 expires=30
//!  -> OBJECT          id=0 sequence=70 priority=4 expires=30
//!  <- SUBSCRIBE_STOP  id=0
//!  -> SUBSCRIBE_RESET id=0 code=206 reason="closed by peer"
//! ```
mod announce;
mod announce_ok;
mod announce_reset;
mod go_away;
mod object;
mod subscribe;
mod subscribe_error;
mod subscribe_fin;
mod subscribe_ok;
mod subscribe_reset;
mod unannounce;
mod unsubscribe;

pub use announce::*;
pub use announce_ok::*;
pub use announce_reset::*;
pub use go_away::*;
pub use object::*;
pub use subscribe::*;
pub use subscribe_error::*;
pub use subscribe_fin::*;
pub use subscribe_ok::*;
pub use subscribe_reset::*;
pub use unannounce::*;
pub use unsubscribe::*;

use crate::coding::{Decode, DecodeError, Encode, EncodeError, VarInt};

use std::fmt;

use crate::coding::{AsyncRead, AsyncWrite};
use crate::setup::Extensions;

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
			pub async fn decode<R: AsyncRead>(r: &mut R, ext: &Extensions) -> Result<Self, DecodeError> {
				let t = VarInt::decode(r).await?;

				match t.into_inner() {
					$($val => {
						let msg = $name::decode(r, ext).await?;
						Ok(Self::$name(msg))
					})*
					_ => Err(DecodeError::InvalidMessage(t)),
				}
			}

			pub async fn encode<W: AsyncWrite>(&self, w: &mut W, ext: &Extensions) -> Result<(), EncodeError> {
				match self {
					$(Self::$name(ref m) => {
						VarInt::from_u32($val).encode(w).await?;
						m.encode(w, ext).await
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
	SubscribeFin = 0xb,
	SubscribeReset = 0xc,

	// ANNOUNCE family, sent by publisher
	Announce = 0x6,
	Unannounce = 0x9,

	// ANNOUNCE family, sent by subscriber
	AnnounceOk = 0x7,
	AnnounceError = 0x8,

	// Misc
	GoAway = 0x10,
}
