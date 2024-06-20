use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::{BroadcastReader, Subscriber};

use crate::Listings;

#[derive(Clone)]
pub struct Session {
	session: web_transport::Session,
	listings: Listings,
}

impl Session {
	pub fn new(session: web_transport::Session, listings: Listings) -> Self {
		Self { session, listings }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let session = self.session.clone();
		let (session, publisher, subscriber) = moq_transfork::Session::accept(session).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(async move { session.run().await.map_err(Into::into) }.boxed());

		if let Some(mut remote) = publisher {
			remote.announce(self.listings.broadcast()).await?;
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_publisher(self, mut remote: Subscriber) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = remote.announced() => {
					let this = self.clone();

					tasks.push(async move {
						let info = announce.name.clone();
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

	async fn serve_announce(mut self, announce: BroadcastReader) -> anyhow::Result<()> {
		self.listings.register(&announce.name)?;
		announce.closed().await?;

		Ok(())
	}
}
