use std::{
	collections::{HashMap, HashSet},
	ops,
	sync::{Arc, Mutex},
};

use anyhow::Context;
use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use url::Url;

use moq_dir::{ListingDelta, ListingReader};
use moq_native::quic;
use moq_transfork::{util::State, Subscribe, Subscriber, Track, TrackWriter};

// TODO split into halves
#[derive(Clone)]
pub struct Remotes {
	root: moq_transfork::Subscriber,
	client: quic::Client,
	myself: Option<String>,
	remotes: Arc<Mutex<HashMap<String, RemoteConsumer>>>,
}

impl Remotes {
	pub fn new(root: moq_transfork::Subscriber, client: quic::Client, myself: Option<String>) -> Self {
		Self {
			root,
			client,
			myself,
			remotes: Default::default(),
		}
	}

	pub async fn run(mut self) -> anyhow::Result<()> {
		let (writer, reader) = Track::new(".", "origins.").produce();
		let subscribe = self.root.subscribe(writer);
		let mut origins = ListingReader::new(reader);
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				update = origins.next() => {
					match update? {
						Some(ListingDelta::Add(host)) => {
							if Some(&host) == self.myself.as_ref() {
								continue
							}

							let (producer, consumer) = Origin::new(&host).produce();

							if let Some(existing) = self.remotes.lock().unwrap().insert(host.clone(), consumer) {
								anyhow::bail!("added duplicate origin: {}", existing.host);
							}

							let this = self.clone();

							tasks.push(async move {
								if let Err(err) = producer.run(this.client).await {
									tracing::warn!("failed running origin: {}, error: {}", host, err)
								}

								this.remotes.lock().unwrap().remove(&host);
							});
						},
						Some(ListingDelta::Rem(host)) => {
							if Some(&host) == self.myself.as_ref() {
								continue
							}

							self.remotes.lock().unwrap().remove(&host);
						}
						None => anyhow::bail!("no origins found"),
					}
				},
				res = subscribe.closed() => return res.map_err(Into::into),
				_ = tasks.next(), if !tasks.is_empty() => {},
			}
		}
	}

	pub fn route(&self, broadcast: &str) -> Option<RemoteConsumer> {
		let active = self.remotes.lock().unwrap();

		for origin in active.values() {
			if origin.contains(broadcast) {
				return Some(origin.clone());
			}
		}

		None
	}
}

pub struct Origin {
	host: String,
}

impl Origin {
	fn new(host: &str) -> Self {
		Self { host: host.to_string() }
	}

	fn produce(self) -> (RemoteProducer, RemoteConsumer) {
		let info = Arc::new(self);
		let state = State::default();

		(
			RemoteProducer::new(info.clone(), state.split()),
			RemoteConsumer::new(info, state),
		)
	}
}

#[derive(Default)]
struct RemoteState {
	broadcasts: HashSet<String>,
	subscriber: Option<Subscriber>, // None when connecting
}

#[derive(Clone)]
pub struct RemoteConsumer {
	info: Arc<Origin>,
	state: State<RemoteState>,
}

impl RemoteConsumer {
	fn new(info: Arc<Origin>, state: State<RemoteState>) -> Self {
		Self { info, state }
	}

	pub fn contains(&self, broadcast: &str) -> bool {
		self.state.lock().broadcasts.contains(broadcast)
	}

	pub fn subscribe(&self, track: TrackWriter) -> Option<Subscribe> {
		let mut subscriber = self.state.lock().subscriber.clone()?;
		Some(subscriber.subscribe(track))
	}
}

impl ops::Deref for RemoteConsumer {
	type Target = Origin;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}

pub struct RemoteProducer {
	info: Arc<Origin>,
	state: State<RemoteState>,
}

impl RemoteProducer {
	fn new(info: Arc<Origin>, state: State<RemoteState>) -> Self {
		Self { info, state }
	}

	async fn run(mut self, client: quic::Client) -> anyhow::Result<()> {
		// Create a URL with the protocol moqf://
		let url = Url::parse(&format!("moqf://{}", self.host))?;
		let session = client.connect(&url).await.context("failed connecting to origin")?;
		let (session, mut subscriber) = Subscriber::connect(session).await?;
		let mut run = session.run().boxed();

		let broadcast = format!(".origin.{}", self.host);
		let (writer, reader) = Track::new(broadcast, "broadcasts").produce();
		let subscribe = subscriber.subscribe(writer);
		let mut broadcasts = ListingReader::new(reader);

		self.state.lock_mut().context("origin closed")?.subscriber = Some(subscriber);

		loop {
			tokio::select! {
				update = broadcasts.next() => {
					let update = update?.context("no broadcasts found")?;
					self.handle(update)?;
				},
				err = subscribe.closed() => return err.map_err(Into::into),
				err = &mut run => return err.map_err(Into::into),
			}
		}
	}

	fn handle(&mut self, update: ListingDelta) -> anyhow::Result<()> {
		let mut state = self.state.lock_mut().context("origin closed")?;
		match update {
			ListingDelta::Add(broadcast) => {
				if !state.broadcasts.insert(broadcast.clone()) {
					anyhow::bail!("duplicate broadcast: {}", broadcast)
				}
			}
			ListingDelta::Rem(broadcast) => {
				if !state.broadcasts.remove(&broadcast) {
					anyhow::bail!("missing broadcast: {}", broadcast)
				}
			}
		}

		Ok(())
	}
}

impl ops::Deref for RemoteProducer {
	type Target = Origin;

	fn deref(&self) -> &Self::Target {
		&self.info
	}
}
