use std::collections::HashMap;

use anyhow::Context;
use clap::Parser;
use moq_lite::{Announced, Broadcast, BroadcastConsumer, Origin};
use moq_native::quic;
use tracing::Instrument;
use url::Url;

#[derive(Clone, Parser)]
pub struct ClusterConfig {
	/// Announce our tracks and discover other origins via this server.
	/// If not provided, then clustering is disabled.
	///
	/// Peers will connect to use via this hostname.
	#[arg(long)]
	pub cluster_root: Option<String>,

	/// Our unique name which we advertise to other origins.
	/// If not provided, then we are a read-only member of the cluster.
	///
	/// Peers will connect to use via this hostname.
	#[arg(long)]
	pub cluster_node: Option<String>,
}

#[derive(Clone)]
pub struct Cluster {
	config: ClusterConfig,
	client: quic::Client,

	// Tracks announced by local clients (users).
	pub locals: Origin,

	// Tracks announced by remote servers (cluster).
	pub remotes: Origin,
}

impl Cluster {
	const ORIGINS: &str = "internal/origins/";

	pub fn new(config: ClusterConfig, client: quic::Client) -> Self {
		Cluster {
			config,
			client,
			locals: Origin::new(),
			remotes: Origin::new(),
		}
	}

	pub fn consume(&self, broadcast: &Broadcast) -> Option<BroadcastConsumer> {
		self.locals
			.consume(broadcast)
			.or_else(|| self.remotes.consume(broadcast))
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let root = self.config.cluster_root.clone();
		let node = self.config.cluster_node.as_ref().map(|node| node.to_string());

		tracing::info!(?root, ?node, "initializing cluster");

		// If we're a node, then we need to announce ourselves as an origin.
		// We do this by creating a "broadcast" with no tracks.
		let myself = if let Some(node) = self.config.cluster_node.as_ref() {
			let origin = format!("internal/origins/{}", node);
			let broadcast = Broadcast { path: origin };
			let producer = broadcast.produce();
			Some(producer)
		} else {
			None
		};

		// If we're using a root node, then we have to connect to it.
		let mut announced = match root.as_ref() {
			Some(root) if Some(root) != node.as_ref() => {
				tracing::info!(?root, "connecting to root");

				// Connect to the root node.
				let root = Url::parse(&format!("https://{}", root)).context("invalid root URL")?;
				let root = self.client.connect(root).await.context("failed to connect to root")?;

				let mut root = moq_lite::Session::connect(root)
					.await
					.context("failed to establish root session")?;

				// Announce ourselves as an origin to the root node.
				if let Some(myself) = &myself {
					root.publish(myself.consume());
				}

				// Subscribe to available origins.
				root.announced(Self::ORIGINS)
			}
			// Otherwise, we're the root node but we still want to connect to other nodes.
			_ => {
				// Announce ourselves as an origin to all connected clients.
				// Technically, we should only announce to cluster clients (not end users), but who cares.
				let mut locals = self.locals.clone();
				if let Some(myself) = &myself {
					locals.publish(myself.consume());
				}

				// Subscribe to the available origins.
				self.locals.announced(Self::ORIGINS)
			}
		};

		// Keep track of the active remotes.
		let mut remotes = HashMap::new();

		// Discover other origins.
		// NOTE: The root node will connect to all other nodes as a client, ignoring the existing (server) connection.
		// This ensures that nodes are advertising a valid hostname before any tracks get announced.
		while let Some(announce) = announced.next().await {
			match announce {
				Announced::Start(broadcast) => {
					let host = broadcast.path.strip_prefix(Self::ORIGINS).unwrap().to_string();

					if Some(&host) == node.as_ref() {
						// Skip ourselves.
						continue;
					}

					tracing::info!(?host, "discovered origin");

					let mut this = self.clone();
					let remote = host.clone();

					let handle = tokio::spawn(
						async move {
							loop {
								if let Err(err) = this.run_remote(&remote).await {
									tracing::error!(?err, "remote error, retrying");
								}

								// TODO smarter backoff
								tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
							}
						}
						.in_current_span(),
					);

					remotes.insert(host, handle);
				}
				Announced::End(broadcast) => {
					let host = broadcast.path.strip_prefix(Self::ORIGINS).unwrap();

					if let Some(handle) = remotes.remove(host) {
						tracing::warn!(?host, "terminating remote");
						handle.abort();
					}
				}
			}
		}

		Ok(())
	}

	#[tracing::instrument("remote", skip_all, err, fields(%host))]
	async fn run_remote(&mut self, host: &str) -> anyhow::Result<()> {
		let url = Url::parse(&format!("https://{}", host)).context("invalid node URL")?;

		// Connect to the remote node.
		let conn = self.client.connect(url).await.context("failed to connect to remote")?;

		let session = moq_lite::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		let mut session1 = session.clone();
		let mut session2 = session.clone();

		tokio::select! {
			// NOTE: We only announce local tracks to remote nodes.
			// Otherwise there would be conflicts and we wouldn't know which node is the origin.
			_ = session1.consume_from(self.locals.clone(), "") => Ok(()),
			// We take any of their remote broadcasts and announce them ourselves.
			_ = session2.publish_to(self.remotes.clone(), "") => Ok(()),
			err = session.closed() => Err(err.into()),
		}
	}
}
