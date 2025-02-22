mod cluster;
mod connection;
mod origins;
mod web;

pub use cluster::*;
pub use connection::*;
pub use origins::*;
pub use web::*;

use anyhow::Context;
use clap::Parser;
use moq_native::quic;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen on this address, both TCP and UDP.
	#[arg(long, default_value = "[::]:443")]
	pub bind: String,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// Cluster configuration.
	#[command(flatten)]
	pub cluster: ClusterConfig,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let bind = tokio::net::lookup_host(config.bind)
		.await
		.context("invalid bind address")?
		.next()
		.context("invalid bind address")?;

	let tls = config.tls.load()?;
	if tls.server.is_none() {
		anyhow::bail!("missing TLS certificates");
	}

	let quic = quic::Endpoint::new(quic::Config { bind, tls: tls.clone() })?;
	let mut server = quic.server.context("missing TLS certificate")?;

	let cluster = Cluster::new(config.cluster.clone(), quic.client);
	let cloned = cluster.clone();
	tokio::spawn(async move { cloned.run().await.expect("cluster failed") });

	// Create a web server too.
	let web = Web::new(WebConfig {
		bind,
		tls,
		cluster: cluster.clone(),
	});

	tokio::spawn(async move {
		web.run().await.expect("failed to run web server");
	});

	tracing::info!(addr = %bind, "listening");

	let mut conn_id = 0;

	while let Some(conn) = server.accept().await {
		let session = Connection::new(conn_id, conn.into(), cluster.clone());
		conn_id += 1;

		tokio::spawn(async move {
			session.run().await.ok();
		});
	}

	Ok(())
}
