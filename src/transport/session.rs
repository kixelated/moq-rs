// TODO implement a wrapper that converts from streams to a API messages.
pub type Session = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, bytes::Bytes>;
