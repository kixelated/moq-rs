use anyhow::Context;
use hang::cmaf::Import;
use hang::moq_lite::Session;
use hang::{BroadcastConsumer, BroadcastProducer};
use moq_native::quic;
use tokio::io::AsyncRead;
use url::Url;

use super::Config;

pub struct BroadcastClient {
	config: Config,
	url: Url,
}

impl BroadcastClient {
	pub fn new(config: Config, url: Url) -> Self {
		Self { url, config }
	}

	pub async fn run<T: AsyncRead + Unpin>(self, input: &mut T) -> anyhow::Result<()> {
		let path = self.url.path().trim_start_matches('/').to_string();
		let broadcast = hang::Broadcast::new(path);

		let producer = BroadcastProducer::new(broadcast.into());
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

		tracing::info!(?self.url, "connecting");

		let session = quic.client.connect(self.url.clone()).await?;
		let mut session = Session::connect(session).await?;

		session.publish(consumer.inner.clone())?;

		tracing::info!("publishing");

		Err(session.closed().await.into())
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
