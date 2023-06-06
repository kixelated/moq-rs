use crate::transport;

use anyhow::Context;
use h3_webtransport::server::WebTransportSession;

pub struct Connection {
	conn: quinn::Connecting,
}

impl Connection {
	pub fn new(conn: quinn::Connecting) -> Self {
		Self { conn }
	}

	pub async fn connect(self) -> anyhow::Result<transport::Session> {
		let conn = self.conn.await.context("failed to accept h3 connection")?;

		let mut conn = h3::server::builder()
			.enable_webtransport(true)
			.enable_connect(true)
			.enable_datagram(true)
			.max_webtransport_sessions(1)
			.send_grease(true)
			.build(h3_quinn::Connection::new(conn))
			.await
			.context("failed to create h3 server")?;

		let (req, stream) = conn
			.accept()
			.await
			.context("failed to accept h3 session")?
			.context("failed to accept h3 request")?;

		let ext = req.extensions();
		anyhow::ensure!(req.method() == http::Method::CONNECT, "expected CONNECT request");
		anyhow::ensure!(
			ext.get::<h3::ext::Protocol>() == Some(&h3::ext::Protocol::WEB_TRANSPORT),
			"expected WebTransport CONNECT"
		);

		let session = WebTransportSession::accept(req, stream, conn)
			.await
			.context("failed to accept WebTransport session")?;

		Ok(session)
	}
}
