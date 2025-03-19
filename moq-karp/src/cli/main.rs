mod client;
mod fingerprint;
mod server;

use client::*;
use fingerprint::*;
use server::*;

use clap::{Parser, Subcommand};
use std::net;

#[derive(Parser, Clone)]
struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// If we're a server or client. If true, we're a server. If false, we're a client to a relay-server.
	#[arg(long)]
	pub server: bool,

	/// If we're publishing or subscribing.
	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	/// Publish a video stream to the provided URL.
	Publish {
		/// The URL must start with `https://` or `http://`.
		///
		/// - If `http` is used, a HTTP fetch to "/fingerprint" is first made to get the TLS certificiate fingerprint (insecure).
		///   The URL is then upgraded to `https`.
		///
		/// - If `https` is used, then A WebTransport connection is made via QUIC to the provided host/port.
		///   The path is used to identify the broadcast, with the rest of the URL (ex. query/fragment) currently ignored.
		url: String,
	},
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	match config.command.clone() {
		Command::Publish { url } => publish(config, url).await,
		//Command::Subscribe => subscribe(session, broadcast).await,
	}
}

#[tracing::instrument(skip_all, fields(?url))]
async fn publish(config: Config, url: String) -> anyhow::Result<()> {
	match config.server {
		true => {
			BroadcastServer::new(config.bind, config.tls, url, tokio::io::stdin())
				.run()
				.await
		}
		false => {
			BroadcastClient::new(config.bind, config.tls, url, tokio::io::stdin())
				.run()
				.await
		}
	}
}

/*
#[tracing::instrument("subscribe", skip_all, err, fields(?broadcast))]
async fn subscribe(session: moq_transfork::Session, broadcast: Broadcast) -> anyhow::Result<()> {
	let broadcast = session.subscribe(broadcast);

	let export = cmaf::Export::init(broadcast, tokio::io::stdout()).await?;
	tracing::info!(catalog = ?export.catalog());

	Ok(export.run().await?)
}
*/
