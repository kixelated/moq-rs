use std::{fs, io, net, path};

use anyhow::Context;
use clap::Parser;
use moq_transport::{Role, SetupServer, Version};
use ring::digest::{digest, SHA256};
use tokio::task::JoinSet;
use warp::Filter;

use moq_warp::{relay::{self}, source};

mod server;

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
	let mut tasks = JoinSet::new();
	tasks.spawn(async move {
		serve.await.unwrap();
	});

	// Create a fake media source from disk.
	let media = source::File::new(args.media).context("failed to open file source")?;

	let broker = relay::broker::Broadcasts::new();
	broker
		.announce("quic.video/demo", media.source())
		.context("failed to announce file source")?;

	let mut tasks = JoinSet::new();
	tasks.spawn(async move {
		media.run().await.unwrap();
	});
	
	// Create a server to actually serve the media
	let config = relay::ServerConfig {
		addr: args.addr,
		cert: args.cert,
		key: args.key,
		broker: broker.clone(),
	};

	let quinn = server::Server::new_quinn_connection(config).unwrap();

	let mut tasks = JoinSet::new();
	loop {
		let broker = broker.clone();
		tokio::select! {
			connect = server::Server::accept_new_webtransport_session(&quinn) => {
				tasks.spawn(async move {
					let client_setup = connect?.accept().await?;
					// TODO: maybe reject setup
					let role = match client_setup.setup().role {
						Role::Publisher => Role::Subscriber,
						Role::Subscriber => Role::Publisher,
						Role::Both => Role::Both,
					};
					let setup_server = SetupServer {
						version: Version::DRAFT_00,
						role,
					};
				
					let session = client_setup.accept(setup_server).await.unwrap();
					let session = relay::Session::from_transport_session(session, broker.clone()).await.unwrap();
					session.run().await?;
					let ret: anyhow::Result<()> = Ok(());
					ret
				});
			}
			res = tasks.join_next(), if !tasks.is_empty() => {
				let res = res.expect("no tasks").expect("task aborted");

				if let Err(err) = res {
					log::error!("session terminated: {:?}", err);
				}
			},
		}
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
	let fingerprint = std::sync::Arc::new(fingerprint);

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
