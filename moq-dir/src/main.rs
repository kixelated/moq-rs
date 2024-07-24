use anyhow::Context;
use clap::Parser;
use futures::{stream::FuturesUnordered, StreamExt};
use moq_transfork::prelude::*;

use std::net;

use moq_native::{quic, tls};

mod connection;
mod listing;
mod listings;

pub use connection::*;
pub use listing::*;
pub use listings::*;

#[derive(Clone, clap::Parser)]
pub struct Config {
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

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;
	let mut quic = quic.server.context("missing server certificate")?;

	let broadcast = Broadcast::new(config.broadcast);
	let listings = Listings::new(broadcast);

	let mut tasks = FuturesUnordered::new();

	loop {
		tokio::select! {
			Some(session) = quic.accept() => {
				let connection = Connection::new(session, listings.clone());
				tasks.push(connection.run());
			},
			_ = tasks.next(), if !tasks.is_empty() => {},
			else => return Ok(()),
		}
	}
}
