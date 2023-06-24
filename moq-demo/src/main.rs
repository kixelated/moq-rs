use std::{fs, io, net, path, sync};

use anyhow::Context;
use clap::Parser;
use ring::digest::{digest, SHA256};
use warp::Filter;

use moq_warp::{relay, source};

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Clone)]
struct Cli {
	/// Listen on this address
	#[arg(short, long, default_value = "[::]:4443")]
	addr: net::SocketAddr,

	/// Use the certificate file at this path
	#[arg(short, long, default_value = "cert/localhost.crt")]
	cert: path::PathBuf,

	/// Use the private key at this path
	#[arg(short, long, default_value = "cert/localhost.key")]
	key: path::PathBuf,

	/// Use the media file at this path
	#[arg(short, long, default_value = "media/fragmented.mp4")]
	media: path::PathBuf,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let args = Cli::parse();

	// Create a web server to serve the fingerprint
	let serve = serve_http(args.clone());

	// Create a fake media source from disk.
	let media = source::File::new(args.media).context("failed to open file source")?;

	let broker = relay::broker::Broadcasts::new();
	broker
		.announce("demo", media.source())
		.context("failed to announce file source")?;

	// Create a server to actually serve the media
	let config = relay::ServerConfig {
		addr: args.addr,
		cert: args.cert,
		key: args.key,
		broker,
	};

	let server = relay::Server::new(config).context("failed to create server")?;

	// Run all of the above
	tokio::select! {
		res = server.run() => res.context("failed to run server"),
		res = media.run() => res.context("failed to run media source"),
		res = serve => res.context("failed to run HTTP server"),
	}
}

// Run a HTTP server using Warp
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
async fn serve_http(args: Cli) -> anyhow::Result<()> {
	// Read the PEM certificate file
	let crt = fs::File::open(&args.cert)?;
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
		.cert_path(args.cert)
		.key_path(args.key)
		.run(args.addr)
		.await;

	Ok(())
}
