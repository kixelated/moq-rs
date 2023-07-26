use anyhow::Context;
use clap::Parser;
use std::net;

mod client;
use client::*;

mod media;
use media::*;

mod publisher;
use publisher::*;

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

	let client = Client::new(config).await?;
	//let media = Media::new().await?;
	let publisher = Publisher::new().await?;

	tokio::select! {
		res = client.run() => res.context("failed to run client")?,
	//	res = media.run() => res.context("failed to run media source")?,
		res = publisher.run() => res.context("failed to run publisher")?,
	}

	Ok(())
}
