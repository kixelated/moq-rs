use std::{
	io::{self, Write},
	net,
};

use anyhow::Context;
use clap::Parser;
use moq_transfork::Broadcast;
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

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	moq_native::log::init();

	let config = Config::parse();

	let tls = config.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	let session = quic.client.connect(&config.url).await?;
	let (session, subscriber) = moq_transfork::Subscriber::connect(session)
		.await
		.context("failed to create MoQ Transport session")?;

	let broadcast = Broadcast::new(&config.name);
	let media = Media::load(subscriber, broadcast).await?;

	tokio::select! {
		res = session.run() => res.context("session error")?,
		res = run_media(media) => res.context("media error")?,
	}

	Ok(())
}

async fn run_media(mut media: Media) -> anyhow::Result<()> {
	let mut stdout = io::stdout();

	while let Some(frame) = media.next().await? {
		stdout.write(&frame)?;
	}

	Ok(())
}
