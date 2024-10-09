//! An fork of the MoQ Transport protocol.
//!
//! MoQ Transfork is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transfork is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-lcurley-moq-transfork/) and [github](https://github.com/kixelated/moq-transfork) for any updates.
mod error;
mod model;
mod session;

pub mod coding;
pub mod message;
pub(crate) mod util;

pub use error::*;
pub use model::*;
pub use session::*;

/// The ALPN used when connecting via QUIC directly.
pub const ALPN: &[u8] = b"moqf-02";
