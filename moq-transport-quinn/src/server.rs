use std::sync::Arc;

use anyhow::Context;
use bytes::Bytes;
use tokio::task::JoinSet;

use moq_transport::{Message, SetupClient, SetupServer};

use super::{Control, Objects};

pub struct Server {
	// The QUIC server, yielding new connections and sessions.
	endpoint: quinn::Endpoint,

	// A list of connections that are completing the WebTransport handshake.
	handshake: JoinSet<anyhow::Result<Connect>>,
}

impl Server {
	pub fn new(endpoint: quinn::Endpoint) -> Self {
		let handshake = JoinSet::new();
		Self { endpoint, handshake }
	}

	// Accept the next WebTransport session.
	pub async fn accept(&mut self) -> anyhow::Result<Connect> {
		loop {
			tokio::select!(
				// Accept the connection and start the WebTransport handshake.
				conn = self.endpoint.accept() => {
					let conn = conn.context("failed to accept connection")?;
					self.handshake.spawn(async move {
						Connecting::new(conn).accept().await
					});
				},
				// Return any mostly finished WebTransport handshakes.
				res = self.handshake.join_next(), if !self.handshake.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					match res {
						Ok(session) => return Ok(session),
						Err(err) => log::warn!("failed to accept session: {:?}", err),
					}
				},
			)
		}
	}
}

struct Connecting {
	conn: quinn::Connecting,
}

impl Connecting {
	pub fn new(conn: quinn::Connecting) -> Self {
		Self { conn }
	}

	pub async fn accept(self) -> anyhow::Result<Connect> {
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

		// Let the application decide if we accept this CONNECT request.
		Ok(Connect { conn, req, stream })
	}
}

// The WebTransport CONNECT has arrived, and we need to decide if we accept it.
pub struct Connect {
	// Inspect to decide whether to accept() or reject() the session.
	req: http::Request<()>,

	conn: h3::server::Connection<h3_quinn::Connection, Bytes>,
	stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl Connect {
	// Expose the received URI
	pub fn uri(&self) -> &http::Uri {
		self.req.uri()
	}

	// Accept the WebTransport session.
	pub async fn accept(self) -> anyhow::Result<Setup> {
		let session = h3_webtransport::server::WebTransportSession::accept(self.req, self.stream, self.conn).await?;
		let session = Arc::new(session);

		let stream = session
			.accept_bi()
			.await
			.context("failed to accept bidi stream")?
			.unwrap();

		let objects = Objects::new(session.clone());

		let stream = match stream {
			h3_webtransport::server::AcceptedBi::BidiStream(_session_id, stream) => stream,
			h3_webtransport::server::AcceptedBi::Request(..) => anyhow::bail!("additional http requests not supported"),
		};

		let mut control = Control::new(stream);
		let setup = match control.recv().await.context("failed to read SETUP")? {
			Message::SetupClient(setup) => setup,
			_ => anyhow::bail!("expected CLIENT SETUP"),
		};

		// Let the application decide if we accept this MoQ session.
		Ok(Setup {
			setup,
			control,
			objects,
		})
	}

	// Reject the WebTransport session with a HTTP response.
	pub async fn reject(mut self, resp: http::Response<()>) -> anyhow::Result<()> {
		self.stream.send_response(resp).await?;
		Ok(())
	}
}

pub struct Setup {
	setup: SetupClient,
	control: Control,
	objects: Objects,
}

impl Setup {
	// Return the setup message we received.
	pub fn setup(&self) -> &SetupClient {
		&self.setup
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup: SetupServer) -> anyhow::Result<Session> {
		self.control.send(setup).await?;
		Ok(Session {
			control: self.control,
			objects: self.objects,
		})
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}

pub struct Session {
	pub control: Control,
	pub objects: Objects,
}

impl Session {
	pub fn split(self) -> (Control, Objects) {
		(self.control, self.objects)
	}
}
