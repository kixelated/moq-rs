use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use url::Url;

use moq_native::quic;
use moq_transfork::prelude::*;
use moq_warp::cmaf;

#[derive(Parser, Clone)]
struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Connect to the given URL starting with https://
	#[arg(long)]
	pub url: Url,

	/// The name of the broadcast
	#[arg(long)]
	pub broadcast: String,

	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	Publish,
	Subscribe,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();
	cli.log.init();

	let tls = cli.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;
	let session = quic.client.connect(&cli.url).await?;
	let client = moq_transfork::Client::new(session);
	let broadcast = Broadcast::new(cli.broadcast);

	match cli.command {
		Command::Subscribe => subscribe(client, broadcast).await,
		Command::Publish => publish(client, broadcast).await,
	}
}

async fn publish(client: moq_transfork::Client, broadcast: Broadcast) -> anyhow::Result<()> {
	let name = broadcast.name.clone();
	let (writer, reader) = broadcast.produce();

	let mut publisher = client.publisher().await?;
	publisher.announce(reader).await.context("failed to announce")?;

	let import = cmaf::Import::init(tokio::io::stdin(), writer).await?;
	tracing::info!(name, catalog = ?import.catalog(), "producing broadcast");

	Ok(import.run().await?)
}

async fn subscribe(client: moq_transfork::Client, broadcast: Broadcast) -> anyhow::Result<()> {
	let name = broadcast.name.clone();
	let subscriber = client.subscriber().await?;
	let broadcast = subscriber.namespace(broadcast)?;

	let export = cmaf::Export::init(broadcast, tokio::io::stdout()).await?;
	tracing::info!(name, catalog = ?export.catalog(), "consuming broadcast");

	Ok(export.run().await?)
}
