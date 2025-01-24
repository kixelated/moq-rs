use moq_native::quic;
use std::net;
use url::Url;

use anyhow::Context;
use clap::Parser;

mod clock;
use moq_transfork::*;

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

	/// The path of the clock track.
	#[arg(long, default_value = "clock")]
	pub path: Vec<String>,

	/// The log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// Whether to publish the clock or consume it.
	#[command(subcommand)]
	pub role: Command,
}

#[derive(Parser, Clone)]
pub enum Command {
	Publish,
	Subscribe,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let config = Config::parse();
	config.log.init();

	let tls = config.tls.load()?;

	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	tracing::info!(url = ?config.url, "connecting to server");

	let session = quic.client.connect(config.url).await?;
	let mut session = moq_transfork::Session::connect(session).await?;

	let path = config.path.into_iter().collect();
	let track = Track::new(path);

	match config.role {
		Command::Publish => {
			let (writer, reader) = track.produce();
			session.publish(reader).context("failed to announce broadcast")?;

			let clock = clock::Publisher::new(writer);

			clock.run().await
		}
		Command::Subscribe => {
			let reader = session.subscribe(track);
			let clock = clock::Subscriber::new(reader);

			clock.run().await
		}
	}
}
