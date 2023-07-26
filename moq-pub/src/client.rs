use crate::media::{self, MapSource};
use http;
use rustls;
use rustls_native_certs;
use std::net;
use std::sync::{Arc, Mutex};
use webtransport_quinn;

use anyhow::Context;

pub struct ClientConfig {
	pub addr: net::SocketAddr,
	pub uri: http::uri::Uri,
}

#[derive(Clone)]
pub struct Client {
	inner: Arc<Mutex<ClientInner>>,
}

pub struct ClientInner {
	session: moq_transport_quinn::Session,
	source: Arc<MapSource>,
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

		let session = webtransport_quinn::connect(&endpoint, &config.uri)
			.await
			.context("failed to create WebTransport session")?;
		let session = moq_transport_quinn::connect(session, moq_transport::Role::Both)
			.await
			.context("failed to create MoQ Transport session")?;
		Ok(Client {
			inner: Arc::new(Mutex::new(ClientInner {
				session,
				source: MapSource::default().into(),
			})),
		})
	}

	pub async fn announce(self, namespace: &str, source: Arc<media::MapSource>) -> anyhow::Result<()> {
		let mut this = self.inner.lock().unwrap();

		// Only allow one souce at a time for now?
		this.source = source;

		// ANNOUNCE the namespace
		this.session
			.send_control
			.send(moq_transport::Announce {
				track_namespace: namespace.to_string(),
			})
			.await?;

		Ok(())
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let this = self.inner.lock().unwrap();

		for track_name in this.source.0.keys() {
			println!("track name: {}", track_name);

			// let track = this.source.0.get_mut(track_name).context("fail")?;
			// track.next_segment(); // etc.
		}

		loop {}
	}
}
