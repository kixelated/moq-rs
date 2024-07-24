use moq_native::quic;
use std::net;
use url::Url;

use anyhow::Context;
use clap::Parser;

mod clock;
use moq_transfork::prelude::*;

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Connect to the given URL starting with https://
	#[arg()]
	pub url: Url,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Publish the current time to the relay, otherwise only subscribe.
	#[arg(long)]
	pub publish: bool,

	/// The name of the clock track.
	#[arg(long, default_value = "clock")]
	pub broadcast: String,

	/// The name of the clock track.
	#[arg(long, default_value = "now")]
	pub track: String,

	/// The log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	log::info!("connecting to server: url={}", config.url);

	let session = quic.client.connect(&config.url).await?;
	let session = moq_transfork::Client::new(session);

	if config.publish {
		let mut publisher = session.publisher().await?;

		let (mut writer, reader) = Broadcast::new(config.broadcast).produce();
		publisher
			.announce(reader)
			.await
			.context("failed to announce broadcast")?;

		let track = writer.create(&config.track, 0).build()?;
		let clock = clock::Publisher::new(track);

		clock.run().await
	} else {
		let mut subscriber = session.subscriber().await?;

		let broadcast = Broadcast::new(config.broadcast);
		let track = Track::new(config.track, 0).build();

		let reader = subscriber.subscribe(broadcast, track).await?;
		let clock = clock::Subscriber::new(reader);

		clock.run().await
	}
}
