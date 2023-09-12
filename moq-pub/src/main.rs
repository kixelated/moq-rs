use anyhow::Context;
use clap::Parser;

mod cli;
mod media;

use cli::*;
use media::*;

use moq_transport::model::broadcast;
use uuid::Uuid;

// TODO: clap complete

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let mut config = Config::parse();

	if config.namespace.is_empty() {
		config.namespace = format!("quic.video/{}", Uuid::new_v4());
	}

	let (publisher, subscriber, _) = broadcast::new(&config.namespace);
	let mut media = Media::new(&config, publisher).await?;

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

	let mut endpoint = quinn::Endpoint::client(config.bind_address)?;
	endpoint.set_default_client_config(quinn_client_config);

	let session = webtransport_quinn::connect(&endpoint, &config.uri)
		.await
		.context("failed to create WebTransport session")?;

	let mut session = moq_transport::Client::publisher(session)
		.await
		.context("failed to create MoQ Transport session")?;

	session.announce(subscriber).context("failed to announce broadcast")?;

	// TODO wait until session.closed() so we fully flush
	media.run().await.context("failed to run media")?;

	Ok(())
}
