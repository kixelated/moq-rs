use crate::media::{self, MapSource};
use http;
use moq_transport::{Object, VarInt};
use rustls;
use rustls_native_certs;
use std::io::Write;
use std::net;
use std::sync::Arc;
use tokio::io::AsyncWriteExt;
use webtransport_quinn;

use anyhow::Context;

pub struct ClientConfig {
	pub addr: net::SocketAddr,
	pub uri: http::uri::Uri,
}

pub struct Client {
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
			session,
			source: Arc::new(MapSource::default()),
		})
	}

	pub async fn announce(&mut self, namespace: &str, source: Arc<media::MapSource>) -> anyhow::Result<()> {
		// Only allow one souce at a time for now?
		self.source = source;

		// ANNOUNCE the namespace
		self.session
			.send_control
			.send(moq_transport::Announce {
				track_namespace: namespace.to_string(),
			})
			.await?;

		Ok(())
	}

	pub async fn run(self) -> anyhow::Result<()> {
		println!("client.run()");
		let mut objects = self.session.send_objects.clone();

		println!("self.source.0.len(): {}", self.source.0.len());
		dbg!(&self.source.0);
		for track_name in self.source.0.keys() {
			println!("track name: {}", track_name);

			let mut track = self.source.0.get(track_name).cloned().context("failed to get track")?;
			println!("track.name: {}", track.name);
			let mut segment = track.next_segment().await?;
			println!("segment: {:?}", &segment);
			let object = Object {
				track: VarInt::from_u32(track_name.parse::<u32>()?),
				group: segment.sequence,
				sequence: VarInt::from_u32(0), // Always zero since we send an entire group as an object
				send_order: segment.send_order,
			};

			let mut stream = objects.open(object).await?;

			// Write each fragment as they are available.
			while let Some(fragment) = segment.fragments.next().await {
				stream.write_all(fragment.as_slice()).await?;
			}
		}
		std::io::stdout().flush()?;
		Ok(())
	}
}
