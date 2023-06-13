use crate::coding::{Decode, Encode};
use crate::{control, object, setup};

use anyhow::Context;
use bytes::Bytes;
use tokio::task::JoinSet;

// Reduce typing because the h3 WebTransport library has quite verbose types.
type Connection = h3::server::Connection<h3_quinn::Connection, Bytes>;
type RequestStream = h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>;
type WebTransportSession = h3_webtransport::server::WebTransportSession<h3_quinn::Connection, Bytes>;
pub type BidiStream = h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>;
pub type SendStream = h3_webtransport::stream::SendStream<h3_quinn::SendStream<Bytes>, Bytes>;
pub type RecvStream = h3_webtransport::stream::RecvStream<h3_quinn::RecvStream, Bytes>;

pub struct Server {
	// The QUIC server, yielding new connections and sessions.
	endpoint: quinn::Endpoint,

	// A list of connections that are completing the WebTransport handshake.
	handshake: JoinSet<anyhow::Result<Accept>>,
}

impl Server {
	pub fn new(endpoint: quinn::Endpoint) -> Self {
		let handshake = JoinSet::new();
		Self { endpoint, handshake }
	}

	// Accept the next WebTransport session.
	pub async fn accept(&mut self) -> anyhow::Result<Accept> {
		loop {
			tokio::select!(
				// Accept the connection and start the WebTransport handshake.
				conn = self.endpoint.accept() => {
					let conn = conn.context("failed to accept connection")?;
					self.handshake.spawn(async move { Self::accept_session(conn).await });
				},
				// Return any mostly finished WebTransport handshakes.
				session = self.handshake.join_next(), if !self.handshake.is_empty() => {
					let _session = session.context("failed to accept session")?;
				},
			)
		}
	}

	async fn accept_session(conn: quinn::Connecting) -> anyhow::Result<Accept> {
		let conn = conn.await.context("failed to accept h3 connection")?;

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
	pub req: http::Request<()>,

	conn: Connection,
	stream: RequestStream,
}

impl Accept {
	// Accept the WebTransport session.
	pub async fn accept(self) -> anyhow::Result<Setup> {
		let transport = h3_webtransport::server::WebTransportSession::accept(self.req, self.stream, self.conn).await?;
		let stream = transport
			.accept_bi()
			.await
			.context("failed to accept bidi stream")?
			.unwrap();

		if let h3_webtransport::server::AcceptedBi::BidiStream(_session_id, mut control) = stream {
			let m = setup::Message::decode(&mut control).await?;
			let setup = match m {
				setup::Message::Client(setup) => setup,
				_ => anyhow::bail!("expected client SETUP"),
			};

			Ok(Setup {
				transport,
				control,
				setup,
			})
		} else {
			anyhow::bail!("multiple SETUP requests");
		}
	}

	// Reject the WebTransport session with a HTTP response.
	pub async fn reject(mut self, resp: http::Response<()>) -> anyhow::Result<()> {
		self.stream.send_response(resp).await?;
		Ok(())
	}
}

pub struct Setup {
	// Inspect to decide if to accept() or reject() the session.
	pub setup: setup::Client,

	transport: WebTransportSession,
	control: BidiStream,
}

impl Setup {
	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup: setup::Server) -> anyhow::Result<Session> {
		let msg = setup::Message::Server(setup);
		msg.encode(&mut self.control).await?;

		Ok(Session {
			transport: self.transport,
			control: self.control,
		})
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}

pub struct Session {
	transport: WebTransportSession,
	control: BidiStream,
}

impl Session {
	pub async fn receive_message(&mut self) -> anyhow::Result<control::Message> {
		control::Message::decode(&mut self.control).await
	}

	pub async fn send_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		msg.encode(&mut self.control).await?;
		Ok(())
	}

	pub async fn receive_data(&mut self) -> anyhow::Result<(object::Header, RecvStream)> {
		let (_session_id, mut stream) = self
			.transport
			.accept_uni()
			.await
			.context("failed to accept uni stream")?
			.context("no uni stream")?;

		let header = object::Header::decode(&mut stream).await?;

		Ok((header, stream))
	}

	pub async fn send_data(&mut self, header: object::Header) -> anyhow::Result<SendStream> {
		let mut stream = self
			.transport
			.open_uni(self.transport.session_id())
			.await
			.context("failed to open uni stream")?;

		header.encode(&mut stream).await?;

		Ok(stream)
	}
}
