use anyhow::Context;
use moq_lite::{BroadcastConsumer, BroadcastProducer, OriginProducer};
use moq_native::quic;
use tracing::Instrument;
use url::Url;

use crate::ClusterConfig;

#[derive(Clone)]
pub struct Cluster {
	config: ClusterConfig,
	client: quic::Client,

	// Tracks announced by local clients (users).
	pub locals: OriginProducer,

	// Tracks announced by remote servers (cluster).
	pub remotes: OriginProducer,
}

impl Cluster {
	const ORIGINS: &str = "internal/origins/";

	pub fn new(config: ClusterConfig, client: quic::Client) -> Self {
		Cluster {
			config,
			client,
			locals: OriginProducer::new(),
			remotes: OriginProducer::new(),
		}
	}

	pub fn get(&self, broadcast: &str) -> Option<BroadcastConsumer> {
		self.locals
			.consume(broadcast)
			.or_else(|| self.remotes.consume(broadcast))
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let root = self.config.root.clone();
		let node = self.config.node.as_ref().map(|node| node.to_string());

		tracing::info!(?root, ?node, "initializing cluster");

		match root.as_ref() {
			// If we're using a root node, then we have to connect to it.
			Some(root) if Some(root) != node.as_ref() => self.run_leaf(root).await,
			// Otherwise, we're the root node so we wait for other nodes to connect to us.
			_ => self.run_root().await,
		}
	}

	async fn run_leaf(self, root: &str) -> anyhow::Result<()> {
		// Create a "broadcast" with no tracks to announce ourselves.
		let noop = BroadcastProducer::new();

		let node = self.config.node.as_ref().map(|node| node.to_string());

		// If we're a node, then we need to announce ourselves as an origin.
		// We do this by creating a "broadcast" with no tracks.
		let myself = node.as_ref().map(|node| format!("internal/origins/{}", node));

		tracing::info!(?root, "connecting to root");

		// Connect to the root node.
		let root = Url::parse(&format!("https://{}", root)).context("invalid root URL")?;
		let root = self.client.connect(root).await.context("failed to connect to root")?;

		let mut root = moq_lite::Session::connect(root)
			.await
			.context("failed to establish root session")?;

		// Announce ourselves as an origin to the root node.
		if let Some(myself) = &myself {
			root.publish(myself, noop.consume());
		}

		// Subscribe to available origins.
		let mut origins = root.consume_prefix(Self::ORIGINS);

		// Discover other origins.
		// NOTE: The root node will connect to all other nodes as a client, ignoring the existing (server) connection.
		// This ensures that nodes are advertising a valid hostname before any tracks get announced.
		while let Some((host, origin)) = origins.next().await {
			if Some(&host) == node.as_ref() {
				// Skip ourselves.
				continue;
			}

			tracing::info!(?host, "discovered origin");

			let mut this = self.clone();
			let remote = host.clone();

			tokio::spawn(
				async move {
					loop {
						let res = tokio::select! {
							biased;
							_ = origin.closed() => break,
							res = this.run_remote(&remote) => res,
						};

						match res {
							Ok(()) => break,
							Err(err) => tracing::error!(?err, "remote error, retrying"),
						}

						// TODO smarter backoff
						tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
					}

					tracing::info!(?host, "origin closed");
				}
				.in_current_span(),
			);
		}

		Ok(())
	}

	async fn run_root(self) -> anyhow::Result<()> {
		tracing::info!("running as root");

		// Literally nothing to do here, because it's handled when accepting connections.

		Ok(())
	}

	#[tracing::instrument("remote", skip_all, err, fields(%host))]
	async fn run_remote(&mut self, host: &str) -> anyhow::Result<()> {
		let url = Url::parse(&format!("https://{}", host)).context("invalid node URL")?;

		// Connect to the remote node.
		let conn = self.client.connect(url).await.context("failed to connect to remote")?;

		let mut session = moq_lite::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		// Publish all of our local broadcasts to the remote.
		let locals = self.locals.consume_all();
		session.publish_all(locals);

		// Consume all of the remote broadcasts.
		let remotes = session.consume_all();
		self.remotes.publish_all(remotes);

		Err(session.closed().await.into())
	}
}
