use std::net;

use anyhow::Context;
use clap::Parser;
use url::Url;

use moq_native::quic;
use moq_sub::media::Media;
use moq_transport::serve::Tracks;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	env_logger::init();

	// Disable tracing so we don't get a bunch of Quinn spam.
	let tracer = tracing_subscriber::FmtSubscriber::builder()
		.with_max_level(tracing::Level::WARN)
		.finish();
	tracing::subscriber::set_global_default(tracer).unwrap();

	let out = tokio::io::stdout();

	let config = Config::parse();
	let tls = config.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: config.bind, tls })?;

	let session = quic.client.connect(&config.url).await?;

	let (session, subscriber) = moq_transport::session::Subscriber::connect(session)
		.await
		.context("failed to create MoQ Transport session")?;

	// Associate empty set of Tracks with provided namespace
	let tracks = Tracks::new(config.name);

	let mut media = Media::new(subscriber, tracks, out).await?;

	tokio::select! {
		res = session.run() => res.context("session error")?,
		res = media.run() => res.context("media error")?,
	}

	Ok(())
}

#[derive(Parser, Clone)]
pub struct Config {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Connect to the given URL starting with https://
	#[arg(value_parser = moq_url)]
	pub url: Url,

	/// The name of the broadcast
	#[arg(long)]
	pub name: String,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,
}

fn moq_url(s: &str) -> Result<Url, String> {
	let url = Url::try_from(s).map_err(|e| e.to_string())?;

	// Make sure the scheme is moq
	if url.scheme() != "https" {
		return Err("url scheme must be https:// for WebTransport".to_string());
	}

	Ok(url)
}
