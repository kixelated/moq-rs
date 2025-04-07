use anyhow::Context;
use hang::cmaf::Import;
use hang::moq_lite::Session;
use hang::BroadcastProducer;
use moq_native::quic;
use tokio::io::AsyncRead;
use url::Url;

use super::Config;

pub struct BroadcastClient<T: AsyncRead + Unpin> {
	config: Config,
	url: String,
	input: T,
}

impl<T: AsyncRead + Unpin> BroadcastClient<T> {
	pub fn new(config: Config, url: String, input: T) -> Self {
		Self { config, url, input }
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		let url = Url::parse(&self.url).context("invalid URL")?;
		let path = url.path().trim_start_matches('/').to_string();

		let session = self.connect(url).await?;

		let mut broadcast = BroadcastProducer::new(path)?;

		let mut import = Import::new(broadcast.clone());
		import
			.init_from(&mut self.input)
			.await
			.context("failed to initialize cmaf from input")?;

		broadcast.add_session(session.clone())?;

		tracing::info!("publishing");

		tokio::select! {
			res = import.read_from(&mut self.input) => Ok(res?),
			res = session.closed() => Err(res.into()),
		}
	}

	async fn connect(&self, url: Url) -> anyhow::Result<Session> {
		let tls = self.config.tls.load()?;
		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls,
		})?;

		tracing::info!(?url, "connecting");

		let session = quic.client.connect(url).await?;
		let session = Session::connect(session).await?;

		Ok(session)
	}
}
