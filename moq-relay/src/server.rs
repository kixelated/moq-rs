use std::{
	fs,
	io::{self, Read},
	sync::Arc,
	time,
};

use anyhow::Context;

use tokio::task::JoinSet;

use crate::{Config, Origin, Session};

pub struct Server {
	quic: quinn::Endpoint,

	// The active connections.
	conns: JoinSet<anyhow::Result<()>>,

	// The map of active broadcasts by path.
	origin: Origin,
}

impl Server {
	// Create a new server
	pub async fn new(config: Config) -> anyhow::Result<Self> {
		// Read the PEM certificate chain
		let certs = fs::File::open(config.cert).context("failed to open cert file")?;
		let mut certs = io::BufReader::new(certs);

		let certs: Vec<rustls::Certificate> = rustls_pemfile::certs(&mut certs)?
			.into_iter()
			.map(rustls::Certificate)
			.collect();

		anyhow::ensure!(!certs.is_empty(), "could not find certificate");

		// Read the PEM private key
		let mut keys = fs::File::open(config.key).context("failed to open key file")?;

		// Read the keys into a Vec so we can try parsing it twice.
		let mut buf = Vec::new();
		keys.read_to_end(&mut buf)?;

		// Try to parse a PKCS#8 key
		// -----BEGIN PRIVATE KEY-----
		let mut keys = rustls_pemfile::pkcs8_private_keys(&mut io::Cursor::new(&buf))?;

		// Try again but with EC keys this time
		// -----BEGIN EC PRIVATE KEY-----
		if keys.is_empty() {
			keys = rustls_pemfile::ec_private_keys(&mut io::Cursor::new(&buf))?
		};

		anyhow::ensure!(!keys.is_empty(), "could not find private key");
		anyhow::ensure!(keys.len() < 2, "expected a single key");

		let key = rustls::PrivateKey(keys.remove(0));

		// Set up a QUIC endpoint that can act as both a client and server.

		// Create a list of acceptable root certificates.
		let mut client_roots = rustls::RootCertStore::empty();

		// Add the platform's native root certificates.
		for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
			client_roots
				.add(&rustls::Certificate(cert.0))
				.context("failed to add root cert")?;
		}

		// For local development, we'll accept our own certificate.
		client_roots
			.add(certs.first().unwrap())
			.context("failed to add our cert to roots")?;

		let mut client_config = rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates(client_roots)
			.with_no_client_auth();

		let mut server_config = rustls::ServerConfig::builder()
			.with_safe_defaults()
			.with_no_client_auth()
			.with_single_cert(certs, key)?;

		server_config.max_early_data_size = u32::MAX;
		client_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];
		server_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.max_idle_timeout(Some(time::Duration::from_secs(10).try_into().unwrap()));
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(4))); // TODO make this smarter
		transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));
		let transport_config = Arc::new(transport_config);

		let mut client_config = quinn::ClientConfig::new(Arc::new(client_config));
		let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(server_config));
		server_config.transport_config(transport_config.clone());
		client_config.transport_config(transport_config);

		// There's a bit more boilerplate to make a generic endpoint.
		let runtime = quinn::default_runtime().context("no async runtime")?;
		let endpoint_config = quinn::EndpointConfig::default();
		let socket = std::net::UdpSocket::bind(config.listen).context("failed to bind UDP socket")?;

		// Create the generic QUIC endpoint.
		let mut quic = quinn::Endpoint::new(endpoint_config, Some(server_config), socket, runtime)
			.context("failed to create QUIC endpoint")?;
		quic.set_default_client_config(client_config);

		let api = config.api.map(|url| {
			log::info!("using moq-api: url={}", url);
			moq_api::Client::new(url)
		});

		if let Some(ref node) = config.node {
			log::info!("advertising origin: url={}", node);
		}

		let origin = Origin::new(api, config.node, quic.clone());
		let conns = JoinSet::new();

		Ok(Self { quic, origin, conns })
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		log::info!("listening on {}", self.quic.local_addr()?);

		loop {
			tokio::select! {
				res = self.quic.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let mut session = Session::new(self.origin.clone());
					self.conns.spawn(async move { session.run(conn).await });
				},
				res = self.conns.join_next(), if !self.conns.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::warn!("connection terminated: {:?}", err);
					}
				},
			}
		}
	}
}
