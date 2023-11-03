//! An implementation of the MoQ Transport protocol.
//!
//! MoQ Transport is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transport is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-ietf-moq-transport/) and [github](https://github.com/moq-wg/moq-transport) for any updates.
//!
//! This implementation has some required extensions until the draft stablizes. See: [Extensions](crate::setup::Extensions)
mod coding;
mod error;

pub mod cache;
pub mod message;
pub mod session;
pub mod setup;

pub use coding::VarInt;
pub use error::MoqError;
