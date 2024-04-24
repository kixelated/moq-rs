use std::{net, sync::Arc, time};

use anyhow::Context;
use clap::Parser;
use url::Url;

use crate::tls;

use futures::future::BoxFuture;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;

#[derive(Parser, Clone)]
pub struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	#[command(flatten)]
	pub tls: tls::Cli,
}

pub struct Config {
	pub bind: net::SocketAddr,
	pub tls: tls::Config,
}

pub struct Endpoint {
	pub client: Client,
	pub server: Option<Server>,
}

pub struct Server {
	quic: quinn::Endpoint,
	accept: FuturesUnordered<BoxFuture<'static, anyhow::Result<web_transport_quinn::Session>>>,
}

#[derive(Clone)]
pub struct Client {
	quic: quinn::Endpoint,
	config: rustls::ClientConfig,
	transport: Arc<quinn::TransportConfig>,
}

impl Endpoint {
	pub fn new(config: Config) -> anyhow::Result<Self> {
		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport = quinn::TransportConfig::default();
		transport.max_idle_timeout(Some(time::Duration::from_secs(10).try_into().unwrap()));
		transport.keep_alive_interval(Some(time::Duration::from_secs(4))); // TODO make this smarter
		transport.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
		transport.mtu_discovery_config(None); // Disable MTU discovery
		let transport = Arc::new(transport);

		let server_config = config.tls.server.map(|mut server| {
			server.alpn_protocols = vec![web_transport_quinn::ALPN.to_vec(), moq_transport::setup::ALPN.to_vec()];
			let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server));
			server_config.transport_config(transport.clone());
			server_config
		});

		// There's a bit more boilerplate to make a generic endpoint.
		let runtime = quinn::default_runtime().context("no async runtime")?;
		let endpoint_config = quinn::EndpointConfig::default();
		let socket = std::net::UdpSocket::bind(config.bind).context("failed to bind UDP socket")?;

		// Create the generic QUIC endpoint.
		let quic = quinn::Endpoint::new(endpoint_config, server_config.clone(), socket, runtime)
			.context("failed to create QUIC endpoint")?;

		let server = server_config.is_some().then(|| Server {
			quic: quic.clone(),
			accept: Default::default(),
		});

		let client = Client {
			quic,
			config: config.tls.client,
			transport,
		};

		Ok(Self { client, server })
	}
}

impl Server {
	pub async fn accept(&mut self) -> Option<web_transport_quinn::Session> {
		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res?;
					self.accept.push(Self::accept_session(conn).boxed());
				}
				res = self.accept.next(), if !self.accept.is_empty() => {
					match res.unwrap() {
						Ok(session) => return Some(session),
						Err(err) => log::warn!("failed to accept QUIC connection: {}", err),
					}
				}
			}
		}
	}

	async fn accept_session(mut conn: quinn::Connecting) -> anyhow::Result<web_transport_quinn::Session> {
		let handshake = conn
			.handshake_data()
			.await?
			.downcast::<quinn::crypto::rustls::HandshakeData>()
			.unwrap();

		let alpn = handshake.protocol.context("missing ALPN")?;
		let alpn = String::from_utf8_lossy(&alpn);
		let server_name = handshake.server_name.unwrap_or_default();

		log::debug!(
			"received QUIC handshake: ip={} alpn={} server={}",
			conn.remote_address(),
			alpn,
			server_name,
		);

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		log::debug!(
			"established QUIC connection: id={} ip={} alpn={} server={}",
			conn.stable_id(),
			conn.remote_address(),
			alpn,
			server_name,
		);

		let session = match alpn.as_bytes() {
			web_transport_quinn::ALPN => {
				// Wait for the CONNECT request.
				let request = web_transport_quinn::accept(conn)
					.await
					.context("failed to receive WebTransport request")?;

				// Accept the CONNECT request.
				request
					.ok()
					.await
					.context("failed to respond to WebTransport request")?
			}
			// A bit of a hack to pretend like we're a WebTransport session
			moq_transport::setup::ALPN => conn.into(),
			_ => anyhow::bail!("unsupported ALPN: {}", alpn),
		};

		Ok(session)
	}

	pub fn local_addr(&self) -> anyhow::Result<net::SocketAddr> {
		self.quic.local_addr().context("failed to get local address")
	}
}

impl Client {
	pub async fn connect(&self, url: &Url) -> anyhow::Result<web_transport::Session> {
		let mut config = self.config.clone();

		// TODO support connecting to both ALPNs at the same time
		config.alpn_protocols = vec![match url.scheme() {
			"https" => web_transport_quinn::ALPN.to_vec(),
			"moqt" => moq_transport::setup::ALPN.to_vec(),
			_ => anyhow::bail!("url scheme must be 'https' or 'moqt'"),
		}];

		let mut config = quinn::ClientConfig::new(Arc::new(config));
		config.transport_config(self.transport.clone());

		let host = url.host().context("invalid DNS name")?.to_string();
		let port = url.port().unwrap_or(443);

		// Look up the DNS entry.
		let addr = tokio::net::lookup_host((host.clone(), port))
			.await
			.context("failed DNS lookup")?
			.next()
			.context("no DNS entries")?;

		let connection = self.quic.connect_with(config, addr, &host)?.await?;

		let session = match url.scheme() {
			"https" => web_transport_quinn::connect_with(connection, url).await?,
			"moqt" => connection.into(),
			_ => unreachable!(),
		};

		Ok(session.into())
	}
}
