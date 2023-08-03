use anyhow::Context;
use clap::Parser;
use std::net;

mod session_runner;
use session_runner::*;

mod media_runner;
use media_runner::*;

mod log_viewer;
use log_viewer::*;

mod media;
use media::*;

#[derive(Parser, Clone)]
struct Cli {
	#[arg(short, long, default_value = "[::]:0")]
	addr: net::SocketAddr,

	#[arg(short, long, default_value = "https://localhost:4443")]
	uri: http::uri::Uri,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	let args = Cli::parse();

	let config = Config {
		addr: args.addr,
		uri: args.uri,
	};

	let mut media = Media::new().await?;
	let mut session_runner = SessionRunner::new(config).await?;
	let mut media_runner = MediaRunner::new(
		session_runner.get_outgoing_senders().await,
		session_runner.get_incoming_receivers().await,
	)
	.await?;
	let mut log_viewer = LogViewer::new(session_runner.get_incoming_receivers().await).await?;
	media_runner.announce("quic.video/moq-pub-foo", media.source()).await?;

	tokio::select! {
		res = media.run() => res.context("failed to run media source")?,
		res = session_runner.run() => res.context("failed to run session runner")?,
		res = log_viewer.run() => res.context("failed to run media source")?,
		res = media_runner.run() => res.context("failed to run client")?,
	}

	Ok(())
}
