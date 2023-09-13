//! Allows a publisher to push updates, automatically caching and fanning it out to any subscribers.
//!
//! The naming scheme doesn't match the spec because it's vague and confusing.
//! The hierarchy is: [broker] -> [broadcast] -> [track] -> [segment] -> [bytes::Bytes]

pub mod broadcast;
pub mod broker;
pub mod segment;
pub mod track;

pub(crate) mod watch;
pub(crate) use watch::*;
