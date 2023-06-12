mod client;
mod server;
mod version;

pub use client::*;
pub use server::*;
pub use version::*;

use super::coding::{Decode, Encode, Size, WithSize};
use bytes::{Buf, BufMut};

// Make a trait that only Client/Server implement.
pub trait Setup: Encode + Decode + Size {}
impl Setup for Client {}
impl Setup for Server {}

// Wrapper to encode a size prefix/suffix.
pub fn encode<B: BufMut, T: Setup>(w: &mut B, t: &T) -> anyhow::Result<()> {
	WithSize::encode(w, t)
}

pub fn decode<B: Buf, T: Setup>(r: &mut B) -> anyhow::Result<T> {
	WithSize::decode(r)
}
