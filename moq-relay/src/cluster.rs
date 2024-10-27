use anyhow::Context;
use clap::Parser;
use moq_native::quic;
use moq_transfork::{Announced, Error, Path, Router, RouterConsumer, RouterProducer, Track};
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

		// If we're a node, then we need to announce our presence.
		let origin = match self.config.cluster_node.as_ref() {
			Some(node) => {
				// Create a track that will be used to announce our presence.
				// We don't actually produce any groups (yet), but it's still a trackso it can be discovered.
				let origin = origins.clone().push(node);
				Track::new(origin).produce().into()
			}
			None => None,
		};

		// If we're using a root node, then we have to connect to it.
		let mut announced = match root.as_ref() {
			Some(root) if Some(root) != node.as_ref() => {
				// Connect to the root node.
				let root = Url::parse(&format!("https://{}", root)).context("invalid root URL")?;
				let root = self.client.connect(&root).await.context("failed to connect to root")?;

				let mut root = moq_transfork::Session::connect(root)
					.await
					.context("failed to establish root session")?;

				// Publish ourselves as an origin.
				if let Some(origin) = origin {
					root.publish(origin.1)?;
				}

				// Subscribe to available origins.
				root.announced_prefix(origins.clone())
			}
			// Otherwise, we're the root node but we still want to connect to other nodes.
			_ => {
				// Subscribe to the available origins.
				self.locals.announced_prefix(origins.clone())
			}
		};

		// Discover other origins.
		while let Some(announce) = announced.next().await {
			// TODO handle Ended?
			if let Announced::Active(path) = &announce {
				tracing::info!(?path, "discovered origin");

				let path = path.clone().strip_prefix(&origins).context("incorrect prefix")?;

				// Extract the hostname from the first part of the path.
				let host = path.first().context("missing node")?.to_string();
				if Some(&host) == node.as_ref() {
					continue;
				}

				tracing::info!(%host, "discovered origin");

				tokio::spawn(self.clone().run_remote(host).in_current_span());
			}
		}

		Ok(())
	}

	#[tracing::instrument("remote", skip_all, err, fields(%host))]
	async fn run_remote(mut self, host: String) -> anyhow::Result<()> {
		// Connect to the remote node.
		let url = Url::parse(&format!("https://{}", host)).context("invalid node URL")?;
		let conn = self.client.connect(&url).await.context("failed to connect to remote")?;

		let mut session = moq_transfork::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		session.route(self.router);

		// NOTE: we don't announce remotes to remotes
		session.announce(self.locals.announced());

		self.remotes.publish(session).await;

		Ok(())
	}
}
