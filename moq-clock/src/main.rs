use moq_native::quic;
use std::net;
use url::Url;

use anyhow::Context;
use clap::Parser;

mod clock;

use moq_transfork::{Broadcast, Produce, Publisher, Subscriber, Track};

#[derive(Parser, Clone)]
pub struct Cli {
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
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	moq_native::log::init();

	let config = Cli::parse();
	let tls = config.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	log::info!("connecting to server: url={}", config.url);

	let session = quic.client.connect(&config.url).await?;

	if config.publish {
		let (session, mut publisher) = Publisher::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let (mut writer, reader) = Broadcast::new(config.broadcast).produce();
		publisher
			.announce(reader)
			.await
			.context("failed to announce broadcast")?;

		let track = writer.create(&config.track, 0).build()?;
		let clock = clock::Publisher::new(track);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	} else {
		let (session, mut subscriber) = Subscriber::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let broadcast = Broadcast::new(config.broadcast);
		let track = Track::new(config.track, 0).build();

		let reader = subscriber.subscribe(broadcast, track).await?;
		let clock = clock::Subscriber::new(reader);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	}

	Ok(())
}
