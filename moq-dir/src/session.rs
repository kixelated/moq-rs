use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::session::{Announced, Publisher, Subscribed, Subscriber};

use crate::Listings;

#[derive(Clone)]
pub struct Session {
	session: web_transport_quinn::Session,
	listings: Listings,
}

impl Session {
	pub fn new(session: web_transport_quinn::Session, listings: Listings) -> Self {
		Self { session, listings }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let session = self.session.clone().into();
		let (session, publisher, subscriber) = moq_transport::session::Session::accept(session).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(async move { session.run().await.map_err(Into::into) }.boxed_local());

		if let Some(remote) = publisher {
			tasks.push(Self::serve_subscriber(self.clone(), remote).boxed_local());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed_local());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_subscriber(self, mut remote: Publisher) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(subscribe) = remote.subscribed() => {
					let this = self.clone();

					tasks.push(async move {
						let info = subscribe.clone();
						log::info!("serving subscribe: {:?}", info);

						if let Err(err) = this.serve_subscribe(subscribe).await {
							log::warn!("failed serving subscribe: {:?}, error: {}", info, err)
						}
					})
				},
				_= tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve_subscribe(self, subscribe: Subscribed) -> anyhow::Result<()> {
		match self.listings.subscribe(&subscribe.namespace, &subscribe.name) {
			Ok(reader) => subscribe.serve(reader.into_inner()).await?,
			Err(err) => subscribe.close(err)?,
		};

		Ok(())
	}

	async fn serve_publisher(self, mut remote: Subscriber) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = remote.announced() => {
					let this = self.clone();

					tasks.push(async move {
						let info = announce.clone();
						log::info!("serving announce: {:?}", info);

						if let Err(err) = this.serve_announce(announce).await {
							log::warn!("failed serving announce: {:?}, error: {}", info, err)
						}
					});
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	async fn serve_announce(mut self, mut announce: Announced) -> anyhow::Result<()> {
		announce.ok()?;

		let registration = self.listings.register(&announce.namespace);
		announce.closed().await?;
		drop(registration);

		Ok(())
	}
}
