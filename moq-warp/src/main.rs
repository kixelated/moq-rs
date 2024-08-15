use std::net;

use anyhow::Context;
use bytes::BytesMut;
use clap::{Parser, Subcommand};
use futures::{stream::FuturesUnordered, StreamExt};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use url::Url;

use moq_native::quic;
use moq_transfork::prelude::*;
use moq_warp::fmp4;

#[derive(Parser, Clone)]
struct Cli {
	/// Listen for UDP packets on the given address.
	#[arg(long, default_value = "[::]:0")]
	pub bind: net::SocketAddr,

	/// Log configuration.
	#[command(flatten)]
	pub log: moq_native::log::Args,

	/// The TLS configuration.
	#[command(flatten)]
	pub tls: moq_native::tls::Args,

	/// Connect to the given URL starting with https://
	#[arg(long)]
	pub url: Url,

	/// The name of the broadcast
	#[arg(long)]
	pub broadcast: String,

	#[command(subcommand)]
	pub command: Command,
}

#[derive(Subcommand, Clone)]
pub enum Command {
	Publish,
	Subscribe,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
	let cli = Cli::parse();
	cli.log.init();

	let tls = cli.tls.load()?;
	let quic = quic::Endpoint::new(quic::Config { bind: cli.bind, tls })?;
	let session = quic.client.connect(&cli.url).await?;
	let client = moq_transfork::Client::new(session);
	let broadcast = Broadcast::new(cli.broadcast);

	match cli.command {
		Command::Subscribe => subscribe(client, broadcast).await,
		Command::Publish => publish(client, broadcast).await,
	}
}

async fn publish(client: moq_transfork::Client, broadcast: Broadcast) -> anyhow::Result<()> {
	let (writer, reader) = broadcast.produce();
	let mut media = fmp4::Producer::new(writer)?;

	let mut publisher = client.publisher().await?;
	publisher.announce(reader).await.context("failed to announce")?;

	let mut input = tokio::io::stdin();
	let mut buf = BytesMut::new();

	loop {
		input.read_buf(&mut buf).await.context("failed to read from stdin")?;
		media.parse(&mut buf).context("failed to parse media")?;
	}
}

async fn subscribe(client: moq_transfork::Client, broadcast: Broadcast) -> anyhow::Result<()> {
	let subscriber = client.subscriber().await?;

	let broadcast = subscriber.namespace(broadcast)?;
	let broadcast = fmp4::BroadcastConsumer::load(broadcast).await?;
	let mut stdout = tokio::io::stdout();

	let catalog = broadcast.catalog.clone();
	let mut tracks = Vec::new();

	for audio in &catalog.audio {
		let track = broadcast.subscribe(audio.track.clone()).await?;
		tracks.push(track);
	}

	for video in &catalog.video {
		let track = broadcast.subscribe(video.track.clone()).await?;
		tracks.push(track);
	}

	stdout.write_all(&broadcast.init.raw).await?;

	while let Some(frame) = read_next(&mut tracks).await? {
		stdout.write_all(&frame.raw).await?;
	}

	Ok(())
}

async fn read_next(tracks: &mut [fmp4::TrackConsumer]) -> anyhow::Result<Option<fmp4::Frame>> {
	let mut futures = FuturesUnordered::new();

	for track in tracks {
		futures.push(track.read());
	}

	loop {
		match futures.next().await {
			Some(Err(err)) => return Err(err.into()),
			Some(Ok(Some(next))) => return Ok(Some(next)),
			Some(Ok(None)) => continue,
			None => return Ok(None),
		};
	}
}
