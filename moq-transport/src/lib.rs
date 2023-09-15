//! An implementation of the MoQ Transport protocol.
//!
//! MoQ Transport is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transport is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) and [github](https://github.com/moq-wg/moq-transport) for any updates.
//!
//! **FORKED**: This is implementation makes extensive changes to the protocol.
//! See [KIXEL_00](crate::setup::Version::KIXEL_00) for a list of differences.
//! Many of these will get merged into the specification, so don't panic.
mod coding;
mod error;

pub mod message;
pub mod model;
pub mod session;
pub mod setup;

pub use coding::VarInt;
pub use error::*;
