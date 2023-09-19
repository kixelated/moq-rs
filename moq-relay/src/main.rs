use std::{fs, io, sync};

use anyhow::Context;
use clap::Parser;
use ring::digest::{digest, SHA256};
use warp::Filter;

mod config;
mod server;
mod session;

pub use config::*;
pub use server::*;
pub use session::*;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	/* TODO disable again after debugging
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();
	*/

	let config = Config::parse();

	// Create a server to actually serve the media
	let server = Server::new(config.clone()).context("failed to create server")?;

	// Run all of the above
	tokio::select! {
		res = server.run() => res.context("failed to run server"),
		res = serve_http(config), if config.fingerprint => res.context("failed to run HTTP server"),
	}
}

// Run a HTTP server using Warp
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
async fn serve_http(config: Config) -> anyhow::Result<()> {
	// Read the PEM certificate file
	let crt = fs::File::open(&config.cert)?;
	let mut crt = io::BufReader::new(crt);

	// Parse the DER certificate
	let certs = rustls_pemfile::certs(&mut crt)?;
	let cert = certs.first().expect("no certificate found");

	// Compute the SHA-256 digest
	let fingerprint = digest(&SHA256, cert.as_ref());
	let fingerprint = hex::encode(fingerprint.as_ref());
	let fingerprint = sync::Arc::new(fingerprint);

	let cors = warp::cors().allow_any_origin();

	// What an annoyingly complicated way to serve a static String
	// I spent a long time trying to find the exact way of cloning and dereferencing the Arc.
	let routes = warp::path!("fingerprint")
		.map(move || (*(fingerprint.clone())).clone())
		.with(cors);

	warp::serve(routes)
		.tls()
		.cert_path(config.cert)
		.key_path(config.key)
		.run(config.bind)
		.await;

	Ok(())
}
