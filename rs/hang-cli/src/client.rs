use anyhow::Context;
use clap::Args;
use hang::cmaf::Import;
use hang::moq_lite;
use hang::{BroadcastConsumer, BroadcastProducer};
use moq_lite::Session;
use moq_native::quic;
use tokio::io::AsyncRead;
use url::Url;

use super::Config;

/// Publish a video stream to the provided URL.
#[derive(Args, Clone)]
pub struct ClientConfig {
	/// The URL of the MoQ server.
	///
	/// The URL must start with `https://` or `http://`.
	/// - If `http` is used, a HTTP fetch to "/certificate.sha256" is first made to get the TLS certificiate fingerprint (insecure).
	///   The URL is then upgraded to `https`.
	///
	/// - If `https` is used, then A WebTransport connection is made via QUIC to the provided host/port.
	///   The path is used to identify the broadcast, with the rest of the URL (ex. query/fragment) currently ignored.
	url: Url,
}

pub struct Client {
	config: Config,
	url: Url,
}

impl Client {
	pub fn new(config: Config, client_config: ClientConfig) -> Self {
		Self {
			config,
			url: client_config.url,
		}
	}

	pub async fn run<T: AsyncRead + Unpin>(self, input: &mut T) -> anyhow::Result<()> {
		let broadcast = hang::Broadcast {
			room: self.config.room.clone(),
			name: self.config.name.clone(),
		};

		let producer = BroadcastProducer::new(broadcast);
		let consumer = producer.consume();

		// Connect to the remote and start parsing stdin in parallel.
		tokio::select! {
			res = self.connect(consumer) => res,
			res = self.publish(producer, input) => res,
		}
	}

	async fn connect(&self, consumer: BroadcastConsumer) -> anyhow::Result<()> {
		let tls = self.config.tls.load()?;
		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls,
		})?;

		tracing::info!(url = %self.url, "connecting");

		let session = quic.client.connect(self.url.clone()).await?;
		let mut session = Session::connect(session).await?;

		session.publish(consumer.inner.clone());

		tracing::info!(room = %self.config.room, name = %self.config.name, "publishing");

		tokio::select! {
			// On ctrl-c, close the session and exit.
			_ = tokio::signal::ctrl_c() => {
				session.close(moq_lite::Error::Cancel);

				// Give it a chance to close.
				tokio::time::sleep(std::time::Duration::from_millis(100)).await;

				Ok(())
			}
			// Otherwise wait for the session to close.
			_ = session.closed() => Err(session.closed().await.into()),
		}
	}

	async fn publish<T: AsyncRead + Unpin>(&self, producer: BroadcastProducer, input: &mut T) -> anyhow::Result<()> {
		let mut import = Import::new(producer);

		import
			.init_from(input)
			.await
			.context("failed to initialize cmaf from input")?;

		tracing::info!("initialized");

		import.read_from(input).await?;

		Ok(())
	}
}
