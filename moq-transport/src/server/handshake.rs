use super::setup::{RecvSetup, SendSetup};
use crate::{control, object, setup};

use anyhow::Context;
use bytes::Bytes;

pub struct Connecting {
	conn: quinn::Connecting,
}

impl Connecting {
	pub fn new(conn: quinn::Connecting) -> Self {
		Self { conn }
	}

	pub async fn accept(self) -> anyhow::Result<Accept> {
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

		// Return the request after validating the bare minimum.
		let accept = Accept { conn, req, stream };

		Ok(accept)
	}
}

// The WebTransport handshake is complete, but we need to decide if we accept it or return 404.
pub struct Accept {
	// Inspect to decide whether to accept() or reject() the session.
	req: http::Request<()>,

	conn: h3::server::Connection<h3_quinn::Connection, Bytes>,
	stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl Accept {
	// Expose the received URI
	pub fn uri(&self) -> &http::Uri {
		self.req.uri()
	}

	// Accept the WebTransport session.
	pub async fn accept(self) -> anyhow::Result<Setup> {
		let transport = h3_webtransport::server::WebTransportSession::accept(self.req, self.stream, self.conn).await?;

		let stream = transport
			.accept_bi()
			.await
			.context("failed to accept bidi stream")?
			.unwrap();

		let transport = object::Transport::new(transport);

		let stream = match stream {
			h3_webtransport::server::AcceptedBi::BidiStream(_session_id, stream) => stream,
			h3_webtransport::server::AcceptedBi::Request(..) => anyhow::bail!("additional http requests not supported"),
		};

		let setup = RecvSetup::new(stream).recv().await?;

		Ok(Setup { transport, setup })
	}

	// Reject the WebTransport session with a HTTP response.
	pub async fn reject(mut self, resp: http::Response<()>) -> anyhow::Result<()> {
		self.stream.send_response(resp).await?;
		Ok(())
	}
}

pub struct Setup {
	setup: SendSetup,
	transport: object::Transport,
}

impl Setup {
	// Return the setup message we received.
	pub fn setup(&self) -> &setup::Client {
		&self.setup.client
	}

	// Accept the session with our own setup message.
	pub async fn accept(self, setup: setup::Server) -> anyhow::Result<(object::Transport, control::Stream)> {
		let control = self.setup.send(setup).await?;
		Ok((self.transport, control))
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}
