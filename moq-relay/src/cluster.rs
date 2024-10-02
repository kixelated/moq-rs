use std::sync::{Arc, Mutex};

use anyhow::Context;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_native::quic;
use moq_transfork::{AnnouncedConsumer, AnnouncedProducer, Broadcast, BroadcastConsumer, BroadcastProducer, Produce};
use url::Url;

use crate::{Config, ListingConsumer, ListingProducer};

#[derive(Clone)]
pub struct Cluster {
	config: Config,
	client: quic::Client,

	local: AnnouncedConsumer,
	remote: AnnouncedProducer,
}

impl Cluster {
	pub fn new(config: Config, client: quic::Client, local: AnnouncedConsumer, remote: AnnouncedProducer) -> Self {
		Cluster {
			config,
			client,
			local,
			remote,
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let root = match self.config.cluster_root {
			Some(root) => root,
			None => return Ok(()),
		};

		let prefix = self.config.cluster_prefix.unwrap_or("origin.".to_string());

		let url = Url::parse(&format!("https://{root}"))?;

		let conn = self
			.client
			.connect(&url)
			.await
			.context("failed to connect to announce server")?;

		let client = moq_transfork::Client::new(conn);
		let (mut publisher, mut subscriber) = client.connect().await.context("failed to establish root session")?;

		let mut tasks = FuturesUnordered::new();

		if let Some(node) = self.config.cluster_node.as_ref() {
			let origin = Broadcast::new(format!("{prefix}{node}")).produce();
			publisher.announce(origin.1).await?;
			tasks.push(Self::run_local(origin.0, self.local).boxed());
		}

		let announced = subscriber
			.announced_prefix(prefix)
			.await
			.context("failed to discover announced origins")?;

		tasks.push(Self::run_remotes(self.remote, announced, self.client).boxed());

		tasks.select_next_some().await
	}

	async fn run_local(mut origin: BroadcastProducer, mut local: AnnouncedConsumer) -> anyhow::Result<()> {
		let primary = origin.insert_track("primary");
		let primary = ListingProducer::new(primary);
		let primary = Arc::new(Mutex::new(primary));

		while let Some(broadcast) = local.next().await {
			primary.lock().unwrap().insert(broadcast.info.name.to_string())?;

			let primary = primary.clone();
			tokio::spawn(async move {
				broadcast.closed().await.ok();
				primary
					.lock()
					.unwrap()
					.remove(&broadcast.info.name)
					.expect("cleanup failed");
			});
		}

		Ok(())
	}

	async fn run_remotes(
		remote: AnnouncedProducer,
		mut announced: AnnouncedConsumer,
		client: quic::Client,
	) -> anyhow::Result<()> {
		while let Some(announce) = announced.next().await {
			let prefix = announced.prefix().to_string();
			let client = client.clone();
			let remote = remote.clone();

			tokio::spawn(async move {
				if let Err(err) = Self::run_remote(remote, announce, prefix, client).await {
					log::error!("failed to run remote: {:?}", err);
				}
			});
		}

		Ok(())
	}

	async fn run_remote(
		mut remote: AnnouncedProducer,
		announce: BroadcastConsumer,
		prefix: String,
		client: quic::Client,
	) -> anyhow::Result<()> {
		let host = announce.info.name.strip_prefix(&prefix).context("invalid prefix")?;
		let url = Url::parse(&format!("https://{}", host))?;

		let conn = client
			.connect(&url)
			.await
			.context("failed to connect to announce server")?;

		let client = moq_transfork::Client::new(conn);

		let origin = client
			.connect_subscriber()
			.await
			.context("failed to establish session")?;

		// Subscribe to the list of tracks being produced.
		let primary = announce.get_track("primary").await?;
		let mut primary = ListingConsumer::new(primary);

		while let Some(listing) = primary.next().await? {
			let broadcast = origin.subscribe(listing.name.clone())?;
			let active = remote.insert(broadcast)?;

			tokio::spawn(async move {
				listing.closed().await.ok();
				drop(active);
			});
		}

		Ok(())
	}
}
