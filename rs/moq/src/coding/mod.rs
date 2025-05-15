//! This module contains encoding and decoding helpers.

mod decode;
mod encode;
mod size;
mod varint;

pub use decode::*;
pub use encode::*;
pub use size::*;
pub use varint::*;

// Re-export the bytes crate
pub use bytes::*;
