use anyhow::Context;
use clap::Parser;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_native::quic;
use moq_transfork::{AnnouncedConsumer, AnnouncedProducer, Broadcast, Path, Produce};
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
		let node = self.config.cluster_node.as_ref().map(|node| node.to_string());

		tracing::info!(?root, ?node, "initializing cluster");

		// We advertise the hostname of origins under this prefix.
		let origins = Path::default().push("internal").push("origins");

		let mut tasks = FuturesUnordered::new();

		// If we're a node, then we need to announce our presence.
		let origin = match self.config.cluster_node.as_ref() {
			Some(node) => {
				// Create a broadcast that will be used to announce our presence.
				// We don't actually produce any tracks (yet), but it's still a broadcast so it can be discovered.
				let origin = origins.clone().push(node);
				Broadcast::new(origin).produce().into()
			}
			None => None,
		};

		// If we're using a root node, then we have to connect to it.
		match root.as_ref() {
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
				let announced = root.announced_prefix(origins.clone());
				tasks.push(Self::run_remotes(self.remote, announced, self.client, node, origins).boxed());
			}
			// Otherwise, we're the root node but we still want to connect to other nodes.
			_ => {
				// Subscribe to the available origins.
				let announced = self.local.with_prefix(origins.clone());
				tasks.push(Self::run_remotes(self.remote, announced, self.client, node, origins).boxed());
			}
		}

		tasks.select_next_some().await
	}

	/*
	#[tracing::instrument("local", skip_all)]
	async fn run_local(mut origin: BroadcastProducer, mut local: AnnouncedConsumer) -> anyhow::Result<()> {
		// Each origin will advertise its broadcasts under this prefix plus their name.
		// This separation makes discovery a little bit easier and more efficient.
		let origin_prefix = Path::default().push("internal").push("origin"); // +hostname

		let primary = origin.insert_track("primary");
		let primary = ListingProducer::new(primary);
		let primary = Arc::new(Mutex::new(primary));

		while let Some(broadcast) = local.next().await {
			primary.lock().unwrap().insert(broadcast.info.path.clone());

			tracing::info!(broadcast = ?broadcast.info);

			let primary = primary.clone();
			tokio::spawn(
				async move {
					broadcast.closed().await.ok();
					primary.lock().unwrap().remove(&broadcast.info.path);
				}
				.in_current_span(),
			);
		}

		Ok(())
	}
	*/

	async fn run_remotes(
		remote: AnnouncedProducer,
		mut announced: AnnouncedConsumer,
		client: quic::Client,
		myself: Option<String>,
		prefix: Path,
	) -> anyhow::Result<()> {
		// Discover other origins.
		while let Some(announce) = announced.next().await {
			let path = announce
				.info
				.path
				.clone()
				.strip_prefix(&prefix)
				.context("incorrect prefix")?;

			// Extract the hostname from the first part of the path.
			let host = path.first().context("missing node")?.to_string();
			if Some(&host) == myself.as_ref() {
				continue;
			}

			let client = client.clone();
			let remote = remote.clone();

			tokio::spawn(Self::run_remote(remote, host, client).in_current_span());
		}

		Ok(())
	}

	#[tracing::instrument("remote", skip_all, err, fields(%host))]
	async fn run_remote(remote: AnnouncedProducer, host: String, client: quic::Client) -> anyhow::Result<()> {
		// Connect to the remote node.
		let url = Url::parse(&format!("https://{}", host)).context("invalid node URL")?;
		let conn = client.connect(&url).await.context("failed to connect to remote")?;

		let session = moq_transfork::Session::connect(conn)
			.await
			.context("failed to establish session")?;

		// Each origin advertises its broadcasts under this prefix plus their name.
		// These are not actually announced to the root node, so we need to connect directly.
		let origin = Path::default().push("internal").push("origin").push(host);
		let mut listings = session.announced_prefix(origin);

		while let Some(listing) = listings.next().await {
			// It's kind of gross, but this `origin` is a fake broadcast.
			// We get the name of the real broadcast by removing the prefix.
			let path = listing.info.path.clone().strip_prefix(listings.prefix()).unwrap();
			let broadcast = session.subscribe(path);

			tracing::info!(broadcast = ?broadcast.info, "available");
			let active = remote.insert(broadcast.clone())?;

			// Spawn a task that will wait until the broadcast is no longer available.
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
