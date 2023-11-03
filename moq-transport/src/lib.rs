//! An implementation of the MoQ Transport protocol.
//!
//! MoQ Transport is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transport is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) and [github](https://github.com/moq-wg/moq-transport) for any updates.
//!
//! **FORKED**: This implementation makes some changes to the protocol.
//! See [KIXEL_01](crate::setup::Version::KIXEL_01) for a list of differences.
//! Many of these will get merged into the specification, so don't panic.
mod coding;
mod error;

pub mod cache;
pub mod message;
pub mod session;
pub mod setup;

pub use coding::VarInt;
pub use error::MoqError;
