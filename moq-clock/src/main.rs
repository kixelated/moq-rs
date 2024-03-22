use std::{fs, io, sync::Arc, time};

use anyhow::Context;
use clap::Parser;

mod cli;
mod clock;

use moq_transport::serve;

// TODO: clap complete

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let config = cli::Config::parse();

	// Create a list of acceptable root certificates.
	let mut roots = rustls::RootCertStore::empty();

	if config.tls_root.is_empty() {
		// Add the platform's native root certificates.
		for cert in rustls_native_certs::load_native_certs().context("could not load platform certs")? {
			roots
				.add(&rustls::Certificate(cert.0))
				.context("failed to add root cert")?;
		}
	} else {
		// Add the specified root certificates.
		for root in &config.tls_root {
			let root = fs::File::open(root).context("failed to open root cert file")?;
			let mut root = io::BufReader::new(root);

			let root = rustls_pemfile::certs(&mut root).context("failed to read root cert")?;
			anyhow::ensure!(root.len() == 1, "expected a single root cert");
			let root = rustls::Certificate(root[0].to_owned());

			roots.add(&root).context("failed to add root cert")?;
		}
	}

	let mut tls_config = rustls::ClientConfig::builder()
		.with_safe_defaults()
		.with_root_certificates(roots)
		.with_no_client_auth();

	// Allow disabling TLS verification altogether.
	if config.tls_disable_verify {
		let noop = NoCertificateVerification {};
		tls_config.dangerous().set_certificate_verifier(Arc::new(noop));
	}

	log::info!("connecting to server: url={}", config.url);

	match config.url.scheme() {
		"https" => {
			tls_config.alpn_protocols = vec![webtransport_quinn::ALPN.to_vec()]; // this one is important
			let client_config = quinn::ClientConfig::new(Arc::new(tls_config));

			let mut endpoint = quinn::Endpoint::client(config.bind)?;
			endpoint.set_default_client_config(client_config);

			let session = webtransport_quinn::connect(&endpoint, &config.url)
				.await
				.context("failed to create WebTransport session")?;

			run(session, config).await
		}
		"moqt" => {
			tls_config.alpn_protocols = vec![moq_transport::setup::ALPN.to_vec()]; // this one is important
			let client_config = quinn::ClientConfig::new(Arc::new(tls_config));

			let mut endpoint = quinn::Endpoint::client(config.bind)?;
			endpoint.set_default_client_config(client_config);

			let session = quictransport_quinn::connect(&endpoint, &config.url)
				.await
				.context("failed to create QUIC Transport session")?;

			run(session, config).await
		}
		_ => anyhow::bail!("unsupported scheme: {}", config.url.scheme()),
	}
}

async fn run<S: webtransport_generic::Session>(session: S, config: cli::Config) -> anyhow::Result<()> {
	if config.publish {
		let (session, publisher) = moq_transport::Publisher::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let (mut broadcast, broadcast_sub) = serve::Broadcast {
			namespace: config.namespace.clone(),
		}
		.produce();

		let track = broadcast.create_track(&config.track)?;
		let clock = clock::Publisher::new(track);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
			res = publisher.serve(broadcast_sub) => res.context("failed to serve broadcast")?,
		}
	} else {
		let (session, mut subscriber) = moq_transport::Subscriber::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let (prod, sub) = serve::Track::new(&config.namespace, &config.track).produce();
		subscriber.subscribe(prod).context("failed to subscribe to track")?;

		let clock = clock::Subscriber::new(sub);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	}

	Ok(())
}

pub struct NoCertificateVerification {}

impl rustls::client::ServerCertVerifier for NoCertificateVerification {
	fn verify_server_cert(
		&self,
		_end_entity: &rustls::Certificate,
		_intermediates: &[rustls::Certificate],
		_server_name: &rustls::ServerName,
		_scts: &mut dyn Iterator<Item = &[u8]>,
		_ocsp_response: &[u8],
		_now: time::SystemTime,
	) -> Result<rustls::client::ServerCertVerified, rustls::Error> {
		Ok(rustls::client::ServerCertVerified::assertion())
	}
}
