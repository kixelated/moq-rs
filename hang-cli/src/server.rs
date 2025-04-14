use super::{Config, FingerprintServer};

use anyhow::Context;
use hang::cmaf::Import;
use hang::moq_lite;
use hang::BroadcastConsumer;
use hang::BroadcastProducer;
use moq_lite::web_transport;
use moq_native::quic;
use tokio::io::AsyncRead;

pub struct BroadcastServer {
	config: Config,
	broadcast: hang::Broadcast,
}

impl BroadcastServer {
	pub fn new(config: Config, broadcast: hang::Broadcast) -> Self {
		Self { config, broadcast }
	}

	pub async fn run<T: AsyncRead + Unpin>(self, input: &mut T) -> anyhow::Result<()> {
		let producer = BroadcastProducer::new(self.broadcast.clone().into());
		let consumer = producer.consume();

		tokio::select! {
			res = self.accept(consumer) => res,
			res = self.publish(producer, input) => res,
		}
	}

	async fn accept(&self, consumer: BroadcastConsumer) -> anyhow::Result<()> {
		let mut config = self.config.clone();

		config.bind = tokio::net::lookup_host(config.bind)
			.await
			.context("invalid bind address")?
			.next()
			.context("invalid bind address")?;

		let tls = config.tls.load()?;
		if tls.server.is_none() {
			anyhow::bail!("missing TLS certificates");
		}

		let quic = quic::Endpoint::new(quic::Config {
			bind: config.bind,
			tls: tls.clone(),
		})?;
		let mut server = quic.server.context("missing TLS certificate")?;

		// Create a web server to serve the fingerprint.
		let web = FingerprintServer::new(config.bind, tls);
		tokio::spawn(async move {
			web.run().await.expect("failed to run web server");
		});

		tracing::info!(addr = %config.bind, "listening");

		let mut conn_id = 0;

		while let Some(session) = server.accept().await {
			let id = conn_id;
			conn_id += 1;

			let consumer = consumer.clone();

			// Handle the connection in a new task.
			tokio::spawn(async move {
				let session: web_transport::Session = session.into();
				let mut session = moq_lite::Session::accept(session)
					.await
					.expect("failed to accept session");

				session.publish(consumer.inner).expect("failed to publish");

				tracing::info!(?id, "accepted");
			});
		}

		Ok(())
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
