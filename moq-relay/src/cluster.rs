use std::{
	collections::HashMap,
	sync::{Arc, Mutex},
};

use anyhow::Context;
use clap::Parser;
use moq_native::quic;
use moq_transfork::{
	Announced, AnnouncedConsumer, AnnouncedProducer, Error, Path, Router, RouterConsumer, RouterProducer, Session,
	Track,
};
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

	announced: AnnouncedProducer,
	routes: Arc<Mutex<HashMap<Path, Session>>>,
	router: RouterConsumer,
}

impl Cluster {
	pub fn new(config: ClusterConfig, client: quic::Client) -> Self {
		let (producer, consumer) = Router { capacity: 1024 }.produce();

		let this = Cluster {
			config,
			client,
			routes: Default::default(),
			router: consumer,
			announced: Default::default(),
		};

		tokio::spawn(this.clone().run_router(producer).in_current_span());

		this
	}

	pub fn router(&self) -> RouterConsumer {
		self.router.clone()
	}

	pub fn announced(&self) -> AnnouncedConsumer {
		self.announced.subscribe()
	}

	// For each received announce, add the session to the routing table.
	pub fn announce(&mut self, mut announced: AnnouncedConsumer, session: moq_transfork::Session) {
		let routes = self.routes.clone();

		tokio::spawn(async move {
			while let Some(announced) = announced.next().await {
				// TODO handle conflicts
				match announced {
					Announced::Active(path) => {
						routes.lock().unwrap().insert(path, session.clone());
					}
					Announced::Ended(path) => {
						routes.lock().unwrap().remove(&path);
					}
				}
			}
		});
	}

	// This is the GUTS of the entire relay.
	// We route any incoming track requests to the appropriate session.
	async fn run_router(self, mut router: RouterProducer) {
		while let Some(req) = router.requested().await {
			tracing::debug!(path = ?req.path, "received request");
			match self.routes.lock().unwrap().get(&req.track.path).cloned() {
				Some(session) => {
					tokio::spawn(async move {
						match session.subscribe(req.track.clone()).await {
							Ok(track) => req.serve(track),
							Err(err) => req.close(err.into()),
						}
					});
				}
				None => {
					req.close(Error::NotFound.into());
				}
			}
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
				self.announced.subscribe_prefix(origins.clone())
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

		session.route(self.router());
		session.announce(self.announced.subscribe());
		self.announce(session.announced(), session);

		Ok(())
	}
}
