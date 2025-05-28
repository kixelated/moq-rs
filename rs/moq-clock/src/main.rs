use url::Url;

use clap::Parser;

mod clock;
use moq_lite::*;

#[derive(Parser, Clone)]
pub struct Config {
	/// Connect to the given URL starting with https://
	#[arg()]
	pub url: Url,

	/// The MoQ client configuration.
	#[command(flatten)]
	pub client: moq_native::ClientConfig,

	/// The path of the clock broadcast.
	#[arg(long, default_value = "clock")]
	pub broadcast: String,

	/// The name of the clock track.
	#[arg(long, default_value = "seconds")]
	pub track: String,

	/// The log configuration.
	#[command(flatten)]
	pub log: moq_native::Log,

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

	let client = config.client.init()?;

	tracing::info!(url = ?config.url, "connecting to server");

	let session = client.connect(config.url).await?;
	let mut session = moq_lite::Session::connect(session).await?;

	let track = Track {
		name: config.track,
		priority: 0,
	};

	match config.role {
		Command::Publish => {
			let broadcast = BroadcastProducer::new();
			let track = broadcast.create(track);
			let clock = clock::Publisher::new(track);

			session.publish(config.broadcast, broadcast.consume());
			clock.run().await
		}
		Command::Subscribe => {
			let broadcast = session.consume(&config.broadcast);
			let track = broadcast.subscribe(&track);
			let clock = clock::Subscriber::new(track);

			clock.run().await
		}
	}
}
