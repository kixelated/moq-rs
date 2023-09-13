//! An implementation of the MoQ Transport protocol.
//!
//! MoQ Transport is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transport is generic and can be used for other live applications.
mod coding;
mod error;

pub mod message;
pub mod model;
pub mod session;
pub mod setup;

pub use coding::VarInt;
pub use error::*;
