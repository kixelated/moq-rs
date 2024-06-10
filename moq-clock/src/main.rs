use moq_native::quic;
use std::net;
use url::Url;

use anyhow::Context;
use clap::Parser;

mod clock;

use moq_transfork::{Broadcast, Publisher, Subscriber, Track};

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
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let config = Cli::parse();
	let tls = config.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	log::info!("connecting to server: url={}", config.url);

	let session = quic.client.connect(&config.url).await?;

	if config.publish {
		let (session, mut publisher) = Publisher::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let (mut writer, reader) = Broadcast::new(&config.broadcast).produce();
		publisher.announce(reader).context("failed to announce broadcast")?;

		let track = writer.create(&config.track).build().unwrap();
		let clock = clock::Publisher::new(track);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	} else {
		let (session, mut subscriber) = Subscriber::connect(session)
			.await
			.context("failed to create MoQ Transport session")?;

		let track = Track::new(&config.broadcast, &config.track).build();

		let reader = subscriber.subscribe(track);
		let clock = clock::Subscriber::new(reader);

		tokio::select! {
			res = session.run() => res.context("session error")?,
			res = clock.run() => res.context("clock error")?,
		}
	}

	Ok(())
}
