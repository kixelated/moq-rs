use anyhow::Context;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::Broadcast;

use std::net;

use moq_native::{quic, tls};

mod listing;
mod listings;
mod session;

pub use listing::*;
pub use listings::*;
pub use session::*;

#[derive(Clone, clap::Parser)]
pub struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: tls::Args,

	/// Aggregate all announcements received with this broadcast prefix.
	/// The list of announcements that match are available as tracks, ending with /.
	///
	/// ex. ANNOUNCE broadcast=public/meeting/12342/alice
	/// ex. TRACK    broadcast=public/ name=meeting/12342/ payload=alice
	///
	/// Any announcements that don't match are ignored.
	#[arg(long, default_value = ".")]
	pub broadcast: String,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	moq_native::log::init();

	let cli = Cli::parse();
	let tls = cli.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;
	let mut quic = quic.server.context("missing server certificate")?;

	let broadcast = Broadcast::new(cli.broadcast);
	let listings = Listings::new(broadcast);

	let mut tasks = FuturesUnordered::new();

	loop {
		tokio::select! {
			Some(session) = quic.accept() => {
				let session = Session::new(session, listings.clone());
				tasks.push(session.run());
			},
			_ = tasks.next(), if !tasks.is_empty() => {},
			else => return Ok(()),
		}
	}
}
