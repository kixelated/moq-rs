//! An fork of the MoQ Transport protocol.
//!
//! MoQ Transfork is a pub/sub protocol over QUIC.
//! While originally designed for live media, MoQ Transfork is generic and can be used for other live applications.
//! The specification is a work in progress and will change.
//! See the [specification](https://datatracker.ietf.org/doc/draft-lcurley-moq-transfork/) and [github](https://github.com/kixelated/moq-drafts) for any updates.
//!
//! The core of this crate is [Session], established with [Session::connect] (client) or [Session::accept] (server).
//! Once you have a session, you can [Session::publish] or [Session::subscribe].
//!
//! # Producing
//! There can be only 1 publisher.
//!
//! - [BroadcastProducer] can create any number of [TrackProducer]s. Each [Track] is produced independently with a specified order/priority.
//! - [TrackProducer] can append any number of [GroupProducer]s, with new subscribers joining at [Group] boundaries (ex. keyframes).
//! - [GroupProducer] can append any number of [Frame]s, either using [GroupProducer::write_frame] (contiguous) or [GroupProducer::create_frame] (chunked).
//! - [FrameProducer] is thus optional, allowing you to specify an upfront size to write multiple chunks.
//!
//! All methods are synchronous and will NOT block.
//! If there are no subscribers, then no data will flow over the network but it will remain in cache.
//! If the session is dropped, then any future writes will error.
//!
//! # Consuming
//! There can be N consumers (via [Clone]), each getting a copy of any requested data.
//!
//! - [BroadcastConsumer] can fetch any number of [TrackConsumer]s. Each [Track] is consumed independently with a specified order/priority.
//! - [TrackConsumer] can fetch any number of [GroupConsumer]s, joining at a [Group] boundary (ex. keyframes).
//! - [GroupConsumer] can fetch any number of [Frame]s, either using [GroupConsumer::read_frame] (contiguous) or [GroupConsumer::next_frame] (chunked).
//! - [FrameConsumer] is thus optional, allowing you to read chunks as they arrive.
//!
//! All methods are asynchronous and will block until data is available.
//! If the publisher disconnects, then the consumer will error.
//! If the publisher is dropped (clean FIN), then the above methods will return [None].
//!
mod error;
mod model;
mod session;

pub mod coding;
pub mod message;
pub use error::*;
pub use model::*;
pub use session::*;

/// The ALPN used when connecting via QUIC directly.
pub const ALPN: &[u8] = b"moqf-02";

/// Export the web_transport crate.
pub use web_transport;
