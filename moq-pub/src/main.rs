use anyhow::Context;
use clap::Parser;
use std::net;
use tokio::time::sleep;

mod client;
use client::*;

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

	let config = ClientConfig {
		addr: args.addr,
		uri: args.uri,
	};

	let mut client = Client::new(config).await?;
	let media = Media::new().await?;
	sleep(std::time::Duration::from_secs(2)).await;
	client.announce("quic.video/moq-pub-foo", media.source()).await?;

	tokio::select! {
		res = media.run() => res.context("failed to run media source")?,
		res = client.run() => res.context("failed to run client")?,
	}

	Ok(())
}
