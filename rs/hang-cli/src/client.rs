use anyhow::Context;
use hang::cmaf::Import;
use hang::moq_lite;
use hang::{BroadcastConsumer, BroadcastProducer};
use moq_lite::Session;
use tokio::io::AsyncRead;
use url::Url;

pub async fn client<T: AsyncRead + Unpin>(
	config: moq_native::ClientConfig,
	url: Url,
	input: &mut T,
) -> anyhow::Result<()> {
	let producer = BroadcastProducer::new();
	let consumer = producer.consume();

	let client = config.init()?;

	// Connect to the remote and start parsing stdin in parallel.
	tokio::select! {
		res = connect(client, url, consumer) => res,
		res = publish(producer, input) => res,
	}
}

async fn connect(client: moq_native::Client, url: Url, consumer: BroadcastConsumer) -> anyhow::Result<()> {
	tracing::info!(%url, "connecting");

	let session = client.connect(url).await?;
	let mut session = Session::connect(session).await?;

	// The path is relative to the URL, so it's empty because we only publish one broadcast.
	session.publish("", consumer.inner.clone());

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

async fn publish<T: AsyncRead + Unpin>(producer: BroadcastProducer, input: &mut T) -> anyhow::Result<()> {
	let mut import = Import::new(producer);

	import
		.init_from(input)
		.await
		.context("failed to initialize cmaf from input")?;

	tracing::info!("initialized");

	import.read_from(input).await?;

	Ok(())
}
