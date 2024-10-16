use std::sync::{Arc, Mutex};

use anyhow::Context;
use clap::Parser;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_native::quic;
use moq_transfork::{AnnouncedConsumer, AnnouncedProducer, Broadcast, BroadcastConsumer, BroadcastProducer, Produce};
use tracing::Instrument;
use url::Url;

use crate::{ListingConsumer, ListingProducer};

#[derive(Clone, Parser)]
pub struct ClusterConfig {
	/// Announce our tracks and discover other origins via this server.
	/// If not provided, then clustering is disabled.
	///
	/// Peers will connect to use via this hostname.
	#[arg(long)]
	pub cluster_root: Option<String>,

	/// Use the provided prefix to discover other origins.
	/// If not provided, then the default is "origin.".
	#[arg(long)]
	pub cluster_prefix: Option<String>,

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

	local: AnnouncedConsumer,
	remote: AnnouncedProducer,
}

impl Cluster {
	pub fn new(
		config: ClusterConfig,
		client: quic::Client,
		local: AnnouncedConsumer,
		remote: AnnouncedProducer,
	) -> Self {
		Cluster {
			config,
			client,
			local,
			remote,
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let root = self.config.cluster_root;
		let prefix = self.config.cluster_prefix.unwrap_or("origin.".to_string());
		let node = self.config.cluster_node.as_ref().map(|node| node.to_string());

		tracing::info!(?root, ?prefix, ?node, "initializing cluster");

		let mut tasks = FuturesUnordered::new();

		// If we're a node, then we need to announce our presence.
		let origin = match self.config.cluster_node.as_ref() {
			Some(node) => {
				// Create a broadcast that will list our tracks.
				let origin = Broadcast::new(format!("{prefix}{node}")).produce();
				tasks.push(Self::run_local(origin.0, self.local.clone()).boxed());
				Some(origin.1)
			}
			None => None,
		};

		// If we're using a root node, then we have to connect to it.
		match root.as_ref() {
			Some(root) if Some(root) != node.as_ref() => {
				let root = Url::parse(&format!("https://{}", root)).context("invalid root URL")?;

				let conn = self.client.connect(&root).await.context("failed to connect to root")?;

				let mut session = moq_transfork::Session::connect(conn)
					.await
					.context("failed to establish root session")?;

				// Publish our broadcast track.
				if let Some(origin) = origin {
					session.publish(origin)?;
				}

				// Subscribe to the list of broadcasts being produced.
				let announced = session.announced_prefix(&prefix);
				tasks.push(Self::run_remotes(self.remote, announced, self.client, node, prefix).boxed());
			}
			// Otherwise, we're the root node but we still want to connect to other nodes.
			_ => {
				let announced = self.local.with_prefix(&prefix);
				tasks.push(Self::run_remotes(self.remote, announced, self.client, node, prefix).boxed());
			}
		}

		tasks.select_next_some().await
	}

	#[tracing::instrument("local", skip_all)]
	async fn run_local(mut origin: BroadcastProducer, mut local: AnnouncedConsumer) -> anyhow::Result<()> {
		let primary = origin.insert_track("primary");
		let primary = ListingProducer::new(primary);
		let primary = Arc::new(Mutex::new(primary));

		while let Some(broadcast) = local.next().await {
			primary.lock().unwrap().insert(broadcast.info.name.to_string())?;

			tracing::info!(broadcast = ?broadcast.info);

			let primary = primary.clone();
			tokio::spawn(
				async move {
					broadcast.closed().await.ok();
					primary
						.lock()
						.unwrap()
						.remove(&broadcast.info.name)
						.expect("cleanup failed");
				}
				.in_current_span(),
			);
		}

		Ok(())
	}

	async fn run_remotes(
		remote: AnnouncedProducer,
		mut announced: AnnouncedConsumer,
		client: quic::Client,
		myself: Option<String>,
		prefix: String,
	) -> anyhow::Result<()> {
		while let Some(announce) = announced.next().await {
			let node = announce
				.info
				.name
				.strip_prefix(&prefix)
				.context("invalid prefix")?
				.to_string();

			if Some(&node) == myself.as_ref() {
				continue;
			}

			let client = client.clone();
			let remote = remote.clone();

			tokio::spawn(Self::run_remote(remote, announce, node, client).in_current_span());
		}

		Ok(())
	}

	#[tracing::instrument("remote", skip_all, err, fields(%node))]
	async fn run_remote(
		remote: AnnouncedProducer,
		announce: BroadcastConsumer,
		node: String,
		client: quic::Client,
	) -> anyhow::Result<()> {
		let url = Url::parse(&format!("https://{}", node)).context("invalid node URL")?;
		let conn = client.connect(&url).await.context("failed to connect to remote")?;

		let session = moq_transfork::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		// Subscribe to the list of tracks being produced.
		let primary = announce.get_track("primary").await?;
		let mut primary = ListingConsumer::new(primary);

		while let Some(listing) = primary.next().await? {
			let broadcast = session.subscribe(listing.name.clone());
			tracing::info!(broadcast = ?broadcast.info, "available");

			let active = remote.insert(broadcast.clone())?;

			tokio::spawn(
				async move {
					listing.closed().await.ok();
					tracing::info!(broadcast = ?broadcast.info, "unavailable");
					drop(active);
				}
				.in_current_span(),
			);
		}

		tracing::info!("done");

		Ok(())
	}
}
