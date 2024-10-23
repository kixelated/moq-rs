use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use url::Url;

use moq_karp::cmaf;
use moq_native::quic;
use moq_transfork::*;

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
	#[arg(long, default_value = "https://relay.quic.video")]
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
	//Subscribe,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();
	cli.log.init();

	let tls = cli.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;

	tracing::info!(url = %cli.url, "connecting");
	let session = quic.client.connect(&cli.url).await?;
	let session = moq_transfork::Session::connect(session).await?;

	let path = Path::default().push(cli.broadcast);
	let broadcast = Broadcast::new(path);

	match cli.command {
		Command::Publish => publish(session, broadcast).await,
		//Command::Subscribe => subscribe(session, broadcast).await,
	}
}

#[tracing::instrument("publish", skip_all, err, fields(?broadcast))]
async fn publish(mut session: moq_transfork::Session, broadcast: Broadcast) -> anyhow::Result<()> {
	let (writer, reader) = broadcast.produce();
	let mut input = tokio::io::stdin();

	let mut import = cmaf::Import::new(writer);
	import.init_from(&mut input).await.context("failed to initialize")?;

	tracing::info!(catalog = ?import.catalog());
	session.publish(reader).context("failed to announce")?;

	Ok(import.read_from(&mut input).await?)
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
