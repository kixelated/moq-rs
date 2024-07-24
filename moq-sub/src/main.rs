use std::{
	io::{self, Write},
	net,
};

use clap::Parser;
use moq_transfork::prelude::*;
use url::Url;

use moq_native::quic;
use moq_sub::media::Media;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Connect to the given URL starting with https://
	#[arg()]
	pub url: Url,

	/// The name of the broadcast
	#[arg(long)]
	pub name: String,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	let session = quic.client.connect(&config.url).await?;
	let subscriber = moq_transfork::Client::new(session).subscriber().await?;

	let broadcast = Broadcast::new(config.name);
	let mut media = Media::load(subscriber, broadcast).await?;

	let mut stdout = io::stdout();

	while let Some(frame) = media.next().await? {
		stdout.write(&frame)?;
	}

	Ok(())
}
