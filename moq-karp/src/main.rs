use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use moq_transfork::{Path, Session};
use url::Url;

use moq_karp::{cmaf, Room};
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

	/// The URL of the broadcast.
	/// The protocol MUST be https:// and there MUST be at least one path fragment.
	/// The last path fragment is the name of the broadcast, the rest is the room name.
	/// ex. https://relay.quic.video/demo/bbb
	#[arg()]
	pub url: Url,
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
	let session = Session::connect(session).await?;

	let path = Path::from_iter(cli.url.path_segments().context("missing path")?);

	match cli.command {
		Command::Publish => publish(session, path).await,
		//Command::Subscribe => subscribe(session, broadcast).await,
	}
}

#[tracing::instrument("publish", skip_all, err, fields(?path))]
async fn publish(session: Session, mut path: Path) -> anyhow::Result<()> {
	let name = path.pop().expect("missing name");
	let room = Room::new(session.clone(), path);
	let broadcast = room.publish(name)?;

	let mut input = tokio::io::stdin();

	let mut import = cmaf::Import::new(broadcast);
	import.init_from(&mut input).await.context("failed to initialize")?;

	tracing::info!(catalog = ?import.catalog(), "publishing");

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
