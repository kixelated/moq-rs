use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use moq_transfork::{Path, Session};
use url::Url;

use moq_karp::{cmaf, BroadcastProducer};
use moq_native::quic;

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

	/// If we're publishing or subscribing.
	#[command(subcommand)]
	pub command: Command,

	/// The URL must start with https://
	pub url: String,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	/// Connect to the server, do nothing else.
	Connect,

	// Publish a video stream.
	Publish,
	//Subscribe,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();
	cli.log.init();

	let tls = cli.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;

	tracing::info!(url = ?cli.url, "connecting");

	let url = Url::parse(&cli.url).context("invalid URL")?;
	let session = quic.client.connect(&url).await?;
	let session = Session::connect(session).await?;

	let path = url.path_segments().context("missing path")?.collect::<Path>();

	match cli.command {
		Command::Connect => return Ok(()),
		Command::Publish => publish(session, path).await,
		//Command::Subscribe => subscribe(session, broadcast).await,
	}
}

#[tracing::instrument(skip_all, fields(?path))]
async fn publish(session: Session, path: Path) -> anyhow::Result<()> {
	// Generate a "unique" ID for this broadcast session.
	// If we crash, then the viewers will automatically reconnect to the new ID.
	let id = std::time::SystemTime::now()
		.duration_since(std::time::UNIX_EPOCH)
		.unwrap()
		.as_millis() as u64;

	let broadcast = BroadcastProducer::new(session.clone(), path, id)?;
	let mut input = tokio::io::stdin();

	let mut import = cmaf::Import::new(broadcast);
	import.init_from(&mut input).await.context("failed to initialize")?;

	tracing::info!("publishing");

	tokio::select! {
		res = import.read_from(&mut input) => Ok(res?),
		res = session.closed() => Err(res.into()),
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
