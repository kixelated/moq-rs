use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::prelude::*;

pub struct Connection {
	session: moq_transfork::Server,

	local: AnnouncedProducer,
	remote: AnnouncedConsumer,
}

impl Connection {
	pub fn new(session: web_transport::Session, local: AnnouncedProducer, remote: AnnouncedConsumer) -> Self {
		Self {
			session: moq_transfork::Server::new(session),
			local,
			remote,
		}
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let (publisher, subscriber) = self.session.accept_any().await?;

		let mut tasks = FuturesUnordered::new();

		if let Some(publisher) = publisher {
			tasks.push(Self::run_consumer(publisher, self.local.subscribe(), self.remote).boxed());
		}

		if let Some(subscriber) = subscriber {
			tasks.push(Self::run_producer(subscriber, self.local).boxed());
		}

		tasks.select_next_some().await
	}

	async fn run_consumer(
		publisher: Publisher,
		local: AnnouncedConsumer,
		remote: AnnouncedConsumer,
	) -> anyhow::Result<()> {
		tokio::select! {
			res = local.forward(publisher.clone()) => res?,
			res = remote.forward(publisher) => res?,
		}

		Ok(())
	}

	async fn run_producer(mut subscriber: Subscriber, mut local: AnnouncedProducer) -> anyhow::Result<()> {
		let mut announced = subscriber.announced().await?;

		while let Some(broadcast) = announced.next().await {
			let active = local.insert(broadcast.clone())?;
			tokio::spawn(async move {
				broadcast.closed().await.ok();
				drop(active);
			});
		}

		Ok(())
	}
}
