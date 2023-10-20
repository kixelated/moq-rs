use anyhow::Context;
use clap::Parser;

mod cli;
use cli::*;

mod media;
use media::*;

use moq_transport::cache::broadcast;

// TODO: clap complete

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let config = Config::parse();

	let (publisher, subscriber) = broadcast::new("");
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

	let mut endpoint = quinn::Endpoint::client(config.bind)?;
	endpoint.set_default_client_config(quinn_client_config);

	log::info!("connecting to relay: url={}", config.url);

	let session = webtransport_quinn::connect(&endpoint, &config.url)
		.await
		.context("failed to create WebTransport session")?;

	let session = moq_transport::session::Client::publisher(session, subscriber)
		.await
		.context("failed to create MoQ Transport session")?;

	// TODO run a task that returns a 404 for all unknown subscriptions.
	tokio::select! {
		res = session.run() => res.context("session error")?,
		res = media.run() => res.context("media error")?,
	}

	Ok(())
}
