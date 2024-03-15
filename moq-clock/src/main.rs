use std::{fs, io, sync::Arc, time};

use anyhow::Context;
use clap::Parser;

mod cli;
mod clock;

use moq_transport::cache::broadcast;
use tokio::net::lookup_host;

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

	tls_config.alpn_protocols = vec!["moq-00".into()]; // this one is important

	let arc_tls_config = std::sync::Arc::new(tls_config);
	let quinn_client_config = quinn::ClientConfig::new(arc_tls_config);

	let mut endpoint = quinn::Endpoint::client(config.bind)?;
	endpoint.set_default_client_config(quinn_client_config);

	log::info!("connecting to relay: url={}", config.url);

	// TODO error on username:password in host
	let host = config.url.host().unwrap().to_string();
	let port = config.url.port().unwrap_or(443);

	// Look up the DNS entry.
	let remote = lookup_host((host.clone(), port)).await.unwrap().next().unwrap();

	// Connect to the server using the addr we just resolved.
	let conn = endpoint.connect(remote, &host)?;
	let conn = conn.await?;

	let (mut publisher, subscriber) = broadcast::new(""); // TODO config.namespace

	if config.publish {
		let session = moq_transport::session::Client::publisher(conn, subscriber)
			.await
			.context("failed to create MoQ Transport session")?;

		let publisher = publisher
			.create_track(&config.track)
			.context("failed to create clock track")?;
		let clock = clock::Publisher::new(publisher);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	} else {
		let session = moq_transport::session::Client::subscriber(conn, publisher)
			.await
			.context("failed to create MoQ Transport session")?;

		let subscriber = subscriber
			.get_track(&config.track)
			.context("failed to get clock track")?;
		let clock = clock::Subscriber::new(subscriber);

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
