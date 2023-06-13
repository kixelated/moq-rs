use crate::coding::{Decode, Encode};
use crate::{control, setup};

use std::{fs, io, net, path, sync, time};

use anyhow::Context;
use bytes::Bytes;
use tokio::task::JoinSet;

pub struct Server {
	// The QUIC server, yielding new connections and sessions.
	server: quinn::Endpoint,

	// A list of connections that are completing the WebTransport handshake.
	handshake: JoinSet<anyhow::Result<Accept>>,
}

pub struct ServerConfig {
	pub addr: net::SocketAddr,
	pub cert: path::PathBuf,
	pub key: path::PathBuf,
}

impl Server {
	// Create a new server
	pub fn new(config: ServerConfig) -> anyhow::Result<Self> {
		// Read the PEM certificate chain
		let certs = fs::File::open(config.cert).context("failed to open cert file")?;
		let mut certs = io::BufReader::new(certs);
		let certs = rustls_pemfile::certs(&mut certs)?
			.into_iter()
			.map(rustls::Certificate)
			.collect();

		// Read the PEM private key
		let keys = fs::File::open(config.key).context("failed to open key file")?;
		let mut keys = io::BufReader::new(keys);
		let mut keys = rustls_pemfile::pkcs8_private_keys(&mut keys)?;

		anyhow::ensure!(keys.len() == 1, "expected a single key");
		let key = rustls::PrivateKey(keys.remove(0));

		let mut tls_config = rustls::ServerConfig::builder()
			.with_safe_default_cipher_suites()
			.with_safe_default_kx_groups()
			.with_protocol_versions(&[&rustls::version::TLS13])
			.unwrap()
			.with_no_client_auth()
			.with_single_cert(certs, key)?;

		tls_config.max_early_data_size = u32::MAX;
		let alpn: Vec<Vec<u8>> = vec![
			b"h3".to_vec(),
			b"h3-32".to_vec(),
			b"h3-31".to_vec(),
			b"h3-30".to_vec(),
			b"h3-29".to_vec(),
		];
		tls_config.alpn_protocols = alpn;

		let mut server_config = quinn::ServerConfig::with_crypto(sync::Arc::new(tls_config));

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		transport_config.congestion_controller_factory(sync::Arc::new(quinn::congestion::BbrConfig::default()));

		server_config.transport = sync::Arc::new(transport_config);
		let server = quinn::Endpoint::server(server_config, config.addr)?;
		let handshake = JoinSet::new();

		Ok(Self { server, handshake })
	}

	// Accept the next WebTransport session.
	pub async fn accept(&mut self) -> anyhow::Result<Accept> {
		loop {
			tokio::select!(
				// Accept the connection and start the WebTransport handshake.
				conn = self.server.accept() => {
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
	conn: h3::server::Connection<h3_quinn::Connection, Bytes>,
	req: http::Request<()>,
	stream: h3::server::RequestStream<h3_quinn::BidiStream<Bytes>, Bytes>,
}

impl Accept {
	// Expose the URI so the app can decide whether to continue.
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
	transport: h3_webtransport::server::WebTransportSession<h3_quinn::Connection, bytes::Bytes>,
	control: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
	setup: setup::Client,
}

impl Setup {
	// Expose the control message provided by the client.
	pub fn setup(&self) -> &setup::Client {
		&self.setup
	}

	// Accept the session with our own setup message.
	pub async fn accept(mut self, setup: setup::Server) -> anyhow::Result<Session> {
		let msg = setup::Message::Server(setup);
		msg.encode(&mut self.control).await?;

		Ok(Session {
			transport: self.transport,
			control: self.control,
			client: self.setup,
			server: msg.try_into().unwrap(),
		})
	}

	pub async fn reject(self) -> anyhow::Result<()> {
		// TODO Close the QUIC connection with an error code.
		Ok(())
	}
}

pub struct Session {
	transport: h3_webtransport::server::WebTransportSession<h3_quinn::Connection, bytes::Bytes>,
	control: h3_webtransport::stream::BidiStream<h3_quinn::BidiStream<Bytes>, Bytes>,
	client: setup::Client,
	server: setup::Server,
}

impl Session {
	pub async fn receive_message(&mut self) -> anyhow::Result<control::Message> {
		control::Message::decode(&mut self.control).await
	}

	/*
	pub async fn receive_data(&mut self) -> anyhow::Result<bytes::Bytes> {
		let stream = self
			.transport
			.accept_uni()
			.await
			.context("failed to accept uni stream")?;

		let (session_id, stream) = stream.context("None uni stream")?;

		let msg = control::Message::read(&mut stream);
		// TODO

		Ok(data)
	}
	*/

	pub async fn send_message(&mut self, msg: control::Message) -> anyhow::Result<()> {
		msg.encode(&mut self.control).await?;
		Ok(())
	}
}
