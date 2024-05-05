use bytes::BytesMut;
use std::net;
use url::Url;

use anyhow::Context;
use clap::Parser;
use tokio::io::AsyncReadExt;

use moq_native::quic;
use moq_pub::Media;
use moq_transport::{serve, session::Publisher};

#[derive(Parser, Clone)]
pub struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Advertise this frame rate in the catalog (informational)
	// TODO auto-detect this from the input when not provided
	#[arg(long, default_value = "24")]
	pub fps: u8,

	/// Advertise this bit rate in the catalog (informational)
	// TODO auto-detect this from the input when not provided
	#[arg(long, default_value = "1500000")]
	pub bitrate: u32,

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
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let cli = Cli::parse();

	let (writer, _, reader) = serve::Tracks::new(cli.name).produce();
	let media = Media::new(writer)?;

	let tls = cli.tls.load()?;

	let quic = quic::Endpoint::new(moq_native::quic::Config {
		bind: cli.bind,
		tls: tls.clone(),
	})?;

	log::info!("connecting to relay: url={}", cli.url);
	let session = quic.client.connect(&cli.url).await?;

	let (session, mut publisher) = Publisher::connect(session)
		.await
		.context("failed to create MoQ Transport publisher")?;

	tokio::select! {
		res = session.run() => res.context("session error")?,
		res = run_media(media) => res.context("media error")?,
		res = publisher.announce(reader) => res.context("publisher error")?,
	}

	Ok(())
}

async fn run_media(mut media: Media) -> anyhow::Result<()> {
	let mut input = tokio::io::stdin();
	let mut buf = BytesMut::new();

	loop {
		input.read_buf(&mut buf).await.context("failed to read from stdin")?;
		media.parse(&mut buf).context("failed to parse media")?;
	}
}
