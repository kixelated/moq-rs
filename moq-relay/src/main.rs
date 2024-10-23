mod cluster;
mod connection;
mod web;

pub use cluster::*;
pub use connection::*;
pub use web::*;

use anyhow::Context;
use std::net;

use clap::Parser;
use moq_native::quic;
use moq_transfork::*;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address
	#[arg(long, default_value = "[::]:443")]
	pub bind: net::SocketAddr,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// Cluster configuration.
	#[command(flatten)]
	pub cluster: ClusterConfig,

	/// Run a web server for debugging purposes.
	#[arg(long)]
	pub dev: bool,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;

	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	if config.dev {
		// Create a web server too.
		// Currently this only contains the certificate fingerprint (for development only).
		let web = Web::new(WebConfig {
			bind: config.bind,
			tls: tls.clone(),
		});

		tokio::spawn(async move {
			web.run().await.expect("failed to run web server");
		});
	}

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;
	let mut server = quic.server.context("missing TLS certificate")?;

	let local = AnnouncedProducer::default();
	let remote = AnnouncedProducer::default();

	let cluster = Cluster::new(config.cluster.clone(), quic.client, local.subscribe(), remote.clone());
	tokio::spawn(async move {
		cluster.run().await.expect("failed to run cluster");
	});

	tracing::info!(addr = %config.bind, "listening");

	let mut conn_id = 0;

	while let Some(conn) = server.accept().await {
		let session = Connection::new(conn_id, conn, local.clone(), remote.subscribe());
		conn_id += 1;

		tokio::spawn(async move {
			session.run().await.ok();
		});
	}

	Ok(())
}
