use http;
use rustls;
use rustls_native_certs;
use std::net;
use webtransport_quinn;

use anyhow::Context;

pub struct ClientConfig {
	pub addr: net::SocketAddr,
	pub uri: http::uri::Uri,
}

pub struct Client {
	client: quinn::Endpoint,
	config: ClientConfig,
}

impl Client {
	pub async fn new(config: ClientConfig) -> anyhow::Result<Self> {
		// Ugh, just let me use my native root certs already
		let mut roots = rustls::RootCertStore::empty();
		for cert in rustls_native_certs::load_native_certs().expect("could not load platform certs") {
			roots.add(&rustls::Certificate(cert.0)).unwrap();
		}

		let mut tls_config = rustls::ClientConfig::builder()
			.with_safe_defaults()
			.with_root_certificates(roots)
			.with_no_client_auth();

		tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()]; // this one is important

		let arc_tls_config = std::sync::Arc::new(tls_config);
		let quinn_client_config = quinn::ClientConfig::new(arc_tls_config);

		let mut endpoint = quinn::Endpoint::client(config.addr)?;
		endpoint.set_default_client_config(quinn_client_config);
		Ok(Client {
			client: endpoint,
			config,
		})
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let session = webtransport_quinn::connect(&self.client, &self.config.uri)
			.await
			.context("failed to create WebTransport session")?;
		let mut session = moq_transport_quinn::connect(session, moq_transport::Role::Both)
			.await
			.context("failed to create MoQ Transport session")?;
		session
			.send_control
			.send(moq_transport::Announce {
				track_namespace: "foo".to_string(),
			})
			.await?;

		loop {}
	}
}
