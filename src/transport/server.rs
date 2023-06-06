use anyhow::Context;
use std::{fs, io, net, path, sync, time};

use crate::transport;

pub struct Server {
	endpoint: quinn::Endpoint,
}

pub struct ServerConfig {
	pub addr: net::SocketAddr,
	pub cert: path::PathBuf,
	pub key: path::PathBuf,
}

impl Server {
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
		let mut transport_config = quinn::TransportConfig::default();
		transport_config.keep_alive_interval(Some(time::Duration::from_secs(2)));
		server_config.transport = sync::Arc::new(transport_config);

		let endpoint = quinn::Endpoint::server(server_config, config.addr)?;

		Ok(Self { endpoint })
	}

	pub async fn accept(&mut self) -> Option<transport::Connection> {
		self.endpoint.accept().await.map(transport::Connection::new)
	}

	pub async fn shutdown(&mut self) {
		self.endpoint.wait_idle().await
	}
}
