use std::time;

// Wraps a QUIC server, automatically mapping from incoming connections to returned Connection objects.
pub trait Server {
	type Connection: Connection;

	// Take ownership of a connection, wrapping it with our own Connection object
	fn accept(&mut self, conn: quiche::Connection) -> anyhow::Result<Self::Connection>;

	fn poll(&mut self) -> anyhow::Result<Option<time::Duration>>;
}

// Wraps a QUIC connection, automatically calling poll() and performing the underlying IO.
pub trait Connection {
	// Return the inner QUIC connection, so that the server can automatically poll it.
	fn conn(&mut self) -> &mut quiche::Connection;

	// NOTE: Do not call conn.poll() or conn.timeout() directly
	fn poll(&mut self) -> anyhow::Result<Option<time::Duration>>;
}
