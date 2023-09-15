use std::{
	collections::HashMap,
	fs, io,
	sync::{Arc, Mutex},
	time,
};

use anyhow::Context;

use moq_transport::model::broadcast;
use tokio::task::JoinSet;

use crate::{Config, Session};

pub struct Server {
	server: quinn::Endpoint,

	// The active connections.
	conns: JoinSet<anyhow::Result<()>>,

	// The map of active broadcasts by path.
	broadcasts: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,
}

impl Server {
	// Create a new server
	pub fn new(config: Config) -> anyhow::Result<Self> {
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

		let mut server_config = quinn::ServerConfig::with_crypto(Arc::new(tls_config));

		// Enable BBR congestion control
		// TODO validate the implementation
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		transport_config.congestion_controller_factory(Arc::new(quinn::congestion::BbrConfig::default()));

		server_config.transport = Arc::new(transport_config);
		let server = quinn::Endpoint::server(server_config, config.bind)?;

		let broadcasts = Default::default();
		let conns = JoinSet::new();

		Ok(Self {
			server,
			broadcasts,
			conns,
		})
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		loop {
			tokio::select! {
				res = self.server.accept() => {
					let conn = res.context("failed to accept QUIC connection")?;
					let mut session = Session::new(self.broadcasts.clone());
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
