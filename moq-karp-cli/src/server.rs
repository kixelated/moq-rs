use super::{Config, FingerprintServer};

use anyhow::Context;
use moq_karp::cmaf::Import;
use moq_karp::moq_transfork;
use moq_karp::BroadcastProducer;
use moq_native::quic;
use moq_native::quic::Server;
use moq_transfork::web_transport;
use tokio::io::AsyncRead;

pub struct BroadcastServer<T: AsyncRead + Unpin> {
	config: Config,
	path: String,
	input: T,
}

impl<T: AsyncRead + Unpin> BroadcastServer<T> {
	pub fn new(config: Config, path: String, input: T) -> Self {
		Self { config, path, input }
	}

	pub async fn run(&mut self) -> anyhow::Result<()> {
		self.config.bind = tokio::net::lookup_host(self.config.bind)
			.await
			.context("invalid bind address")?
			.next()
			.context("invalid bind address")?;

		let tls = self.config.tls.load()?;
		if tls.server.is_none() {
			anyhow::bail!("missing TLS certificates");
		}

		let quic = quic::Endpoint::new(quic::Config {
			bind: self.config.bind,
			tls: tls.clone(),
		})?;
		let server = quic.server.context("missing TLS certificate")?;

		// Create a web server to serve the fingerprint.
		let web = FingerprintServer::new(self.config.bind, tls);
		tokio::spawn(async move {
			web.run().await.expect("failed to run web server");
		});

		// Create the broadcast
		let broadcast = BroadcastProducer::new(self.path.clone())?;

		let mut import = Import::new(broadcast.clone());
		import
			.init_from(&mut self.input)
			.await
			.context("failed to initialize cmaf from input")?;

		self.accept(server, broadcast)?;
		import.read_from(&mut self.input).await?; // Blocking method

		Ok(())
	}

	fn accept(&mut self, mut server: Server, mut broadcast: BroadcastProducer) -> anyhow::Result<()> {
		tracing::info!(addr = %self.config.bind, "listening");

		let mut conn_id = 0;

		tokio::spawn(async move {
			while let Some(conn) = server.accept().await {
				// Create a new connection
				let session: web_transport::Session = conn.into();
				let transfork_session = moq_transfork::Session::accept(session)
					.await
					.expect("failed to accept session");

				conn_id += 1;
				broadcast.add_session(transfork_session).expect("failed to add session");

				tracing::info!(id = conn_id.clone(), "accepted");
			}
		});

		Ok(())
	}
}
