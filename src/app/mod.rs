mod message;
mod server;
mod session;

pub use server::{Server, ServerConfig};

// Reduce the amount of typing
type WebTransportSession = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, bytes::Bytes>;
