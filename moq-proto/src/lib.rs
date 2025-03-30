//! An fork of the MoQ Transport protocol.
//!
//! MoQ Transfork is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transfork is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-lcurley-moq-transfork/) and [github](https://github.com/kixelated/moq-drafts) for any updates.
//!
//! This crate contains any runtime agnostic components.
//! It's currently super simple but will be expanded as Tokio becomes more of a hindrence.
//!
pub mod coding;
pub mod message;
