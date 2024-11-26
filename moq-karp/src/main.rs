use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use url::Url;

use moq_karp::{cmaf, Member};
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

	/// The broadcast address.
	#[arg()]
	pub addr: Url,
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

	let path = moq_transfork::Path::from_iter(cli.addr.path_segments().context("no broadcast path")?);

	tracing::info!(url = %cli.addr, "connecting");
	let session = quic.client.connect(&cli.addr).await?;
	let session = moq_transfork::Session::connect(session).await?;

	match cli.command {
		Command::Publish => publish(session, path).await,
		//Command::Subscribe => subscribe(session, broadcast).await,
	}
}

#[tracing::instrument("publish", skip_all, err, fields(?path))]
async fn publish(session: moq_transfork::Session, path: moq_transfork::Path) -> anyhow::Result<()> {
	let user = Member::new(session.clone(), path).produce().broadcast_now()?;

	let mut input = tokio::io::stdin();

	let mut import = cmaf::Import::new(user);
	import.init_from(&mut input).await.context("failed to initialize")?;

	tracing::info!(catalog = ?import.catalog());

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
