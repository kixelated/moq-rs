//! Low-level message sent over the wire, as defined in the specification.
//!
//! TODO Update this
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
mod announce_cancel;
mod announce_error;
mod announce_ok;
mod filter_type;
mod go_away;
mod publisher;
mod subscribe;
mod subscribe_done;
mod subscribe_error;
mod subscribe_ok;
mod subscribe_update;
mod subscriber;
mod track_status;
mod track_status_request;
mod unannounce;
mod unsubscribe;

pub use announce::*;
pub use announce_cancel::*;
pub use announce_error::*;
pub use announce_ok::*;
pub use filter_type::*;
pub use go_away::*;
pub use publisher::*;
pub use subscribe::*;
pub use subscribe_done::*;
pub use subscribe_error::*;
pub use subscribe_ok::*;
pub use subscribe_update::*;
pub use subscriber::*;
pub use track_status::*;
pub use track_status_request::*;
pub use unannounce::*;
pub use unsubscribe::*;

use crate::coding::{Decode, DecodeError, Encode, EncodeError};
use std::fmt;

// Use a macro to generate the message types rather than copy-paste.
// This implements a decode/encode method that uses the specified type.
macro_rules! message_types {
    {$($name:ident = $val:expr,)*} => {
		/// All supported message types.
		#[derive(Clone)]
		pub enum Message {
			$($name($name)),*
		}

		impl Decode for Message {
			fn decode<R: bytes::Buf>(r: &mut R) -> Result<Self, DecodeError> {
				let t = u64::decode(r)?;

				match t {
					$($val => {
						let msg = $name::decode(r)?;
						Ok(Self::$name(msg))
					})*
					_ => Err(DecodeError::InvalidMessage(t)),
				}
			}
		}

		impl Encode for Message {
			fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
				match self {
					$(Self::$name(ref m) => {
						self.id().encode(w)?;
						m.encode(w)
					},)*
				}
			}
		}

		impl Message {
			pub fn id(&self) -> u64 {
				match self {
					$(Self::$name(_) => {
						$val
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
	SubscribeUpdate = 0x2,
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
	AnnounceCancel = 0xc,

	// TRACK_STATUS_REQUEST, sent by subscriber
	TrackStatusRequest = 0xd,

	// TRACK_STATUS, sent by publisher
	TrackStatus = 0xe,

	// Misc
	GoAway = 0x10,
}

/// Track Status Codes
/// https://www.ietf.org/archive/id/draft-ietf-moq-transport-04.html#name-track_status
#[derive(Clone, Debug, PartialEq, Copy)]
pub enum TrackStatusCode {
	// 0x00: The track is in progress, and subsequent fields contain the highest group and object ID for that track.
	InProgress = 0x00,
	// 0x01: The track does not exist. Subsequent fields MUST be zero, and any other value is a malformed message.
	DoesNotExist = 0x01,
	// 0x02: The track has not yet begun. Subsequent fields MUST be zero. Any other value is a malformed message.
	NotYetBegun = 0x02,
	// 0x03: The track has finished, so there is no "live edge." Subsequent fields contain the highest Group and object ID known.
	Finished = 0x03,
	// 0x04: The sender is a relay that cannot obtain the current track status from upstream. Subsequent fields contain the largest group and object ID known.
	Relay = 0x04,
}

impl Decode for TrackStatusCode {
	fn decode<B: bytes::Buf>(r: &mut B) -> Result<Self, DecodeError> {
		match u64::decode(r)? {
			0x00 => Ok(Self::InProgress),
			0x01 => Ok(Self::DoesNotExist),
			0x02 => Ok(Self::NotYetBegun),
			0x03 => Ok(Self::Finished),
			0x04 => Ok(Self::Relay),
			_ => Err(DecodeError::InvalidTrackStatusCode),
		}
	}
}

impl Encode for TrackStatusCode {
	fn encode<W: bytes::BufMut>(&self, w: &mut W) -> Result<(), EncodeError> {
		match self {
			Self::InProgress => (0x00_u64).encode(w),
			Self::DoesNotExist => (0x01_u64).encode(w),
			Self::NotYetBegun => (0x02_u64).encode(w),
			Self::Finished => (0x03_u64).encode(w),
			Self::Relay => (0x04_u64).encode(w),
		}
	}
}
