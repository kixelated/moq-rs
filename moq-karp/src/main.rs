use std::net;

use anyhow::Context;
use clap::{Parser, Subcommand};
use moq_transfork::{Path, Session};
use url::Url;

use moq_karp::{cmaf, BroadcastProducer};
use moq_native::quic;

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

async fn connect(config: &Config, url: &str) -> anyhow::Result<(Session, Path)> {
	let tls = config.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	tracing::info!(?url, "connecting");

	let url = Url::parse(url).context("invalid URL")?;
	let path = url.path_segments().context("missing path")?.collect::<Path>();

	let session = quic.client.connect(url).await?;
	let session = Session::connect(session).await?;

	Ok((session, path))
}

#[tracing::instrument(skip_all, fields(?url))]
async fn publish(config: Config, url: String) -> anyhow::Result<()> {
	let (session, path) = connect(&config, &url).await?;
	let broadcast = BroadcastProducer::new(session.clone(), path)?;
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
