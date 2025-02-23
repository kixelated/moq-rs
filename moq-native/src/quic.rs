use std::{net, sync::Arc, time};

use anyhow::Context;
use clap::Parser;
use url::Url;

use crate::tls;

use futures::future::BoxFuture;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;

use web_transport::quinn as web_transport_quinn;

#[derive(Parser, Clone)]
pub struct Args {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	#[command(flatten)]
	pub tls: tls::Args,
}

impl Default for Args {
	fn default() -> Self {
		Self {
			bind: "[::]:0".parse().unwrap(),
			tls: Default::default(),
		}
	}
}

impl Args {
	pub fn load(&self) -> anyhow::Result<Config> {
		let tls = self.tls.load()?;
		Ok(Config { bind: self.bind, tls })
	}
}

pub struct Config {
	pub bind: net::SocketAddr,
	pub tls: tls::Config,
}

pub struct Endpoint {
	pub client: Client,
	pub server: Option<Server>,
}

impl Endpoint {
	pub fn new(config: Config) -> anyhow::Result<Self> {
		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport = quinn::TransportConfig::default();
		transport.max_idle_timeout(Some(time::Duration::from_secs(9).try_into().unwrap()));
		transport.keep_alive_interval(Some(time::Duration::from_secs(4))); // TODO make this smarter
		transport.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
		transport.mtu_discovery_config(None); // Disable MTU discovery
		let transport = Arc::new(transport);

		let mut server_config = None;

		if let Some(mut config) = config.tls.server {
			config.alpn_protocols = vec![web_transport::quinn::ALPN.to_vec(), moq_transfork::ALPN.to_vec()];
			config.key_log = Arc::new(rustls::KeyLogFile::new());

			let config: quinn::crypto::rustls::QuicServerConfig = config.try_into()?;
			let mut config = quinn::ServerConfig::with_crypto(Arc::new(config));
			config.transport_config(transport.clone());

			server_config = Some(config);
		}

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

pub struct Server {
	quic: quinn::Endpoint,
	accept: FuturesUnordered<BoxFuture<'static, anyhow::Result<web_transport_quinn::Session>>>,
}

impl Server {
	pub async fn accept(&mut self) -> Option<web_transport_quinn::Session> {
		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res?;
					self.accept.push(Self::accept_session(conn).boxed());
				}
				Some(res) = self.accept.next() => {
					if let Ok(session) = res {
						return Some(session)
					}
				}
			}
		}
	}

	async fn accept_session(conn: quinn::Incoming) -> anyhow::Result<web_transport_quinn::Session> {
		let mut conn = conn.accept()?;

		let handshake = conn
			.handshake_data()
			.await?
			.downcast::<quinn::crypto::rustls::HandshakeData>()
			.unwrap();

		let alpn = handshake.protocol.context("missing ALPN")?;
		let alpn = String::from_utf8(alpn).context("failed to decode ALPN")?;
		let host = handshake.server_name.unwrap_or_default();

		tracing::debug!(%host, ip = %conn.remote_address(), %alpn, "accepting");

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		let span = tracing::Span::current();
		span.record("id", conn.stable_id()); // TODO can we get this earlier?

		let session = match alpn.as_bytes() {
			web_transport::quinn::ALPN => {
				// Wait for the CONNECT request.
				let request = web_transport::quinn::Request::accept(conn)
					.await
					.context("failed to receive WebTransport request")?;

				// Accept the CONNECT request.
				request
					.ok()
					.await
					.context("failed to respond to WebTransport request")?
			}
			// A bit of a hack to pretend like we're a WebTransport session
			moq_transfork::ALPN => conn.into(),
			_ => anyhow::bail!("unsupported ALPN: {}", alpn),
		};

		Ok(session)
	}

	pub fn local_addr(&self) -> anyhow::Result<net::SocketAddr> {
		self.quic.local_addr().context("failed to get local address")
	}
}

#[derive(Clone)]
pub struct Client {
	quic: quinn::Endpoint,
	config: rustls::ClientConfig,
	transport: Arc<quinn::TransportConfig>,
}

impl Client {
	pub async fn connect(&self, mut url: Url) -> anyhow::Result<web_transport_quinn::Session> {
		let mut config = self.config.clone();

		let host = url.host().context("invalid DNS name")?.to_string();
		let port = url.port().unwrap_or(443);

		// Look up the DNS entry.
		let ip = tokio::net::lookup_host((host.clone(), port))
			.await
			.context("failed DNS lookup")?
			.next()
			.context("no DNS entries")?;

		if url.scheme() == "http" {
			// Perform a HTTP request to fetch the certificate fingerprint.
			let mut fingerprint = url.clone();
			fingerprint.set_path("/fingerprint");

			tracing::warn!(url = %fingerprint, "performing insecure HTTP request for certificate");

			let resp = reqwest::get(fingerprint.as_str())
				.await
				.context("failed to fetch fingerprint")?
				.error_for_status()
				.context("fingerprint request failed")?;

			let fingerprint = resp.text().await.context("failed to read fingerprint")?;
			let fingerprint = hex::decode(fingerprint.trim()).context("invalid fingerprint")?;

			let verifier = tls::FingerprintVerifier::new(config.crypto_provider().clone(), fingerprint);
			config.dangerous().set_certificate_verifier(Arc::new(verifier));

			url.set_scheme("https").expect("failed to set scheme");
		}

		let alpn = match url.scheme() {
			"https" => web_transport::quinn::ALPN,
			"moqf" => moq_transfork::ALPN,
			_ => anyhow::bail!("url scheme must be 'http', 'https', or 'moqf'"),
		};

		// TODO support connecting to both ALPNs at the same time
		config.alpn_protocols = vec![alpn.to_vec()];
		config.key_log = Arc::new(rustls::KeyLogFile::new());

		let config: quinn::crypto::rustls::QuicClientConfig = config.try_into()?;
		let mut config = quinn::ClientConfig::new(Arc::new(config));
		config.transport_config(self.transport.clone());

		tracing::debug!(%url, %ip, alpn = %String::from_utf8_lossy(alpn), "connecting");

		let connection = self.quic.connect_with(config, ip, &host)?.await?;
		tracing::Span::current().record("id", connection.stable_id());

		let session = match url.scheme() {
			"https" => web_transport::quinn::Session::connect(connection, &url).await?,
			"moqf" => connection.into(),
			_ => unreachable!(),
		};

		Ok(session)
	}
}
