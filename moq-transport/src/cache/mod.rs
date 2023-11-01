//! Allows a publisher to push updates, automatically caching and fanning it out to any subscribers.
//!
//! The hierarchy is: [broadcast] -> [track] -> [segment] -> [fragment] -> [Bytes](bytes::Bytes)
//!
//! The naming scheme doesn't match the spec because it's more strict, and bikeshedding of course:
//!
//! - [broadcast] is kinda like "track namespace"
//! - [track] is "track"
//! - [segment] is "group" but MUST use a single stream.
//! - [fragment] is "object" but MUST have the same properties as the segment.

pub mod broadcast;
mod error;
pub mod fragment;
pub mod segment;
pub mod track;

pub(crate) mod watch;
pub(crate) use watch::*;

pub use error::*;
