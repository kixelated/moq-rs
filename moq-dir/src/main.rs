use anyhow::Context;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};

use std::net;

use moq_native::{quic, tls};

mod listing;
mod session;

pub use listing::*;
pub use session::*;

#[derive(Clone, clap::Parser)]
pub struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:4443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: tls::Cli,

	/// Aggregate all announcements received with this namespace prefix.
	/// The list of announcements that match are available as tracks, ending with /.
	///
	/// ex. ANNOUNCE namespace=public/meeting/12342/alice
	/// ex. TRACK    namespace=public/ name=meeting/12342/ payload=alice
	///
	/// Any announcements that don't match are ignored.
	#[arg(long, default_value = "/")]
	pub namespace: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let cli = Cli::parse();
	let tls = cli.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;
	let mut quic = quic.server.context("missing server certificate")?;

	let listings = Listings::new(cli.namespace);

	let mut tasks = FuturesUnordered::new();

	loop {
		tokio::select! {
			res = quic.accept() => {
				let session = res.context("failed to accept QUIC connection")?;
				let session = Session::new(session, listings.clone());

				tasks.push(async move {
					if let Err(err) = session.run().await {
						log::warn!("session terminated: {}", err);
					}
				});
			},
			res = tasks.next(), if !tasks.is_empty() => res.unwrap(),
		}
	}
}
