use std::io::BufReader;
use std::net::SocketAddr;
use std::{fs::File, sync::Arc};

use moq::{session, transport};

use clap::Parser;
use ring::digest::{digest, SHA256};
use warp::Filter;

/// Search for a pattern in a file and display the lines that contain it.
#[derive(Parser, Clone)]
struct Cli {
	/// Listen on this address
	#[arg(short, long, default_value = "[::]:4443")]
	addr: String,

	/// Use the certificate file at this path
	#[arg(short, long, default_value = "cert/localhost.crt")]
	cert: String,

	/// Use the private key at this path
	#[arg(short, long, default_value = "cert/localhost.key")]
	key: String,

	/// Use the media file at this path
	#[arg(short, long, default_value = "media/fragmented.mp4")]
	media: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let moq_args = Cli::parse();
	let http_args = moq_args.clone();

	// TODO return result instead of panicing
	tokio::task::spawn(async move { run_transport(moq_args).unwrap() });

	run_http(http_args).await
}

// Run the WebTransport server using quiche.
fn run_transport(args: Cli) -> anyhow::Result<()> {
	let server_config = transport::Config {
		addr: args.addr,
		cert: args.cert,
		key: args.key,
	};

	let mut server = transport::Server::<session::Session>::new(server_config).unwrap();
	server.run()
}

// Run a HTTP server using Warp
// TODO remove this when Chrome adds support for self-signed certificates using WebTransport
async fn run_http(args: Cli) -> anyhow::Result<()> {
	let addr: SocketAddr = args.addr.parse()?;

	// Read the PEM certificate file
	let crt = File::open(&args.cert)?;
	let mut crt = BufReader::new(crt);

	// Parse the DER certificate
	let certs = rustls_pemfile::certs(&mut crt)?;
	let cert = certs.first().expect("no certificate found");

	// Compute the SHA-256 digest
	let fingerprint = digest(&SHA256, cert.as_ref());
	let fingerprint = hex::encode(fingerprint.as_ref());
	let fingerprint = Arc::new(fingerprint);

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
		.run(addr)
		.await;

	Ok(())
}
