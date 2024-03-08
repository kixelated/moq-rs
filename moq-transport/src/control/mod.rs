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
mod announce_error;
mod announce_ok;
mod go_away;
mod message;
mod subscribe;
mod subscribe_done;
mod subscribe_error;
mod subscribe_ok;
mod unannounce;
mod unsubscribe;

pub use announce::*;
pub use announce_error::*;
pub use announce_ok::*;
pub use go_away::*;
pub use message::*;
pub use subscribe::*;
pub use subscribe_done::*;
pub use subscribe_error::*;
pub use subscribe_ok::*;
pub use unannounce::*;
pub use unsubscribe::*;
