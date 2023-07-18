use moq_warp::relay::broker;

use std::{fs, io, net, path, sync, time};

use anyhow::Context;

use tokio::task::JoinSet;

pub struct Server {
	server: quinn::Endpoint,

	// The media sources.
	broker: broker::Broadcasts,

	// The active connections.
	conns: JoinSet<anyhow::Result<()>>,
}

pub struct ServerConfig {
	pub addr: net::SocketAddr,
	pub cert: path::PathBuf,
	pub key: path::PathBuf,
	pub broker: broker::Broadcasts,
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
		tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()];

		let mut server_config = quinn::ServerConfig::with_crypto(sync::Arc::new(tls_config));

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		transport_config.congestion_controller_factory(sync::Arc::new(quinn::congestion::BbrConfig::default()));

		server_config.transport = sync::Arc::new(transport_config);
		let server = quinn::Endpoint::server(server_config, config.addr)?;
		let broker = config.broker;

		let conns = JoinSet::new();

		Ok(Self { server, broker, conns })
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				res = self.server.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let broker = self.broker.clone();

					self.conns.spawn(async move { Self::handle(conn, broker).await });
				},
				res = self.conns.join_next(), if !self.conns.is_empty() => {
					let res = res.expect("no tasks").expect("task aborted");
					if let Err(err) = res {
						log::error!("connection terminated: {:?}", err);
					}
				},
			}
		}
	}

	async fn handle(conn: quinn::Connecting, _broker: broker::Broadcasts) -> anyhow::Result<()> {
		let conn = conn.await.context("failed to establish QUIC connection")?;

		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		let _session = moq_transport_quinn::accept(session, moq_transport::Role::Both)
			.await
			.context("failed to perform MoQ handshake")?;

		Ok(())
	}
}
