//! A MoQ Transport session, on top of a WebTransport session, on top of a QUIC connection.
//!
//! The handshake is relatively simple but split into different steps.
//! All of these handshakes slightly differ depending on if the endpoint is a client or server.
//! 1. Complete the QUIC handhake.
//! 2. Complete the WebTransport handshake.
//! 3. Complete the MoQ handshake.
//!
//! Use [Client] or [Server] for the MoQ handshake depending on the endpoint.
//! Then, decide if you want to create a [Publisher] or [Subscriber], or both (TODO).
//!
//! A [Publisher] can announce broadcasts, which will automatically be served over the network.
//! A [Subscriber] can subscribe to broadcasts, which will automatically be served over the network.

mod client;
mod control;
mod error;
mod publisher;
mod server;
mod subscriber;

pub use client::*;
pub(crate) use control::*;
pub use error::*;
pub use publisher::*;
pub use server::*;
pub use subscriber::*;
