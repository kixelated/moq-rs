use std::collections::HashMap;

use anyhow::Context;
use clap::Parser;
use moq_native::quic;
use moq_transfork::{Announced, AnnouncedProducer, Error, Path, Router, RouterConsumer, RouterProducer};
use tracing::Instrument;
use url::Url;

use crate::Origins;

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
	pub locals: Origins,

	// Tracks announced by remote servers (cluster).
	pub remotes: Origins,

	// Used to route incoming requests to the origins above.
	pub router: RouterConsumer,
}

impl Cluster {
	pub fn new(config: ClusterConfig, client: quic::Client) -> Self {
		let (producer, consumer) = Router { capacity: 1024 }.produce();

		let this = Cluster {
			config,
			client,
			router: consumer,
			locals: Origins::new(),
			remotes: Origins::new(),
		};

		tokio::spawn(this.clone().run_router(producer).in_current_span());

		this
	}

	// This is the GUTS of the entire relay.
	// We route any incoming track requests to the appropriate session.
	async fn run_router(self, mut router: RouterProducer) {
		while let Some(req) = router.requested().await {
			let origin = if let Some(origin) = self.locals.route(&req.track.path).clone() {
				origin
			} else if let Some(origin) = self.remotes.route(&req.track.path).clone() {
				origin
			} else {
				req.close(Error::NotFound);
				continue;
			};

			let track = origin.subscribe(req.track.clone());
			req.serve(track)
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let root = self.config.cluster_root.clone();
		let node = self.config.cluster_node.as_ref().map(|node| node.to_string());

		tracing::info!(?root, ?node, "initializing cluster");

		// We advertise the hostname of origins under this prefix.
		let origins = Path::default().push("internal").push("origins");

		// If we're a node, then we need to announce ourselves as an origin.
		let mut myself = AnnouncedProducer::new();
		if let Some(node) = self.config.cluster_node.as_ref() {
			let origin = origins.clone().push(node);
			myself.announce(origin);
		}

		// If we're using a root node, then we have to connect to it.
		let mut announced = match root.as_ref() {
			Some(root) if Some(root) != node.as_ref() => {
				tracing::info!(?root, "connecting to root");

				// Connect to the root node.
				let root = Url::parse(&format!("https://{}", root)).context("invalid root URL")?;
				let root = self.client.connect(root).await.context("failed to connect to root")?;

				let mut root = moq_transfork::Session::connect(root)
					.await
					.context("failed to establish root session")?;

				// Announce ourselves as an origin to the root node.
				root.announce(myself.subscribe());

				tracing::info!(?origins, "waiting for prefix");

				// Subscribe to available origins.
				root.announced(origins.clone())
			}
			// Otherwise, we're the root node but we still want to connect to other nodes.
			_ => {
				// Announce ourselves as an origin to all connected clients.
				// Technically, we should only announce to cluster clients (not end users), but who cares.
				let mut locals = self.locals.clone();
				tokio::spawn(async move {
					// Run this in a background task so we don't block the main loop.
					// (it will never exit)
					locals.announce(myself.subscribe(), None).await
				});

				tracing::info!(?node, "acting as root");

				// Subscribe to the available origins.
				self.locals.announced_prefix(origins.clone())
			}
		};

		// Keep track of the active remotes.
		let mut remotes = HashMap::new();

		// Discover other origins.
		// NOTE: The root node will connect to all other nodes as a client, ignoring the existing (server) connection.
		// This ensures that nodes are advertising a valid hostname before any tracks get announced.
		while let Some(announce) = announced.next().await {
			match announce {
				Announced::Active(path) => {
					// Extract the hostname from the first part of the path.
					let host = path.first().context("missing node")?.to_string();
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
				Announced::Ended(path) => {
					let host = path.first().context("missing node")?.to_string();
					if let Some(handle) = remotes.remove(&host) {
						tracing::warn!(?host, "terminating remote");
						handle.abort();
					}
				}
				Announced::Live => {
					// Ignore.
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

		let mut session = moq_transfork::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		session.route(self.router.clone());

		// NOTE: We only announce local tracks to remote nodes.
		// Otherwise there would be conflicts and we wouldn't know which node is the origin.
		session.announce(self.locals.announced());

		// Add any tracks to the list of remotes for routing.
		let all = session.announced(Path::default());
		self.remotes.announce(all, Some(session.clone())).await;

		Ok(())
	}
}
