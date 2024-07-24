use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::prelude::*;

use crate::Listings;

#[derive(Clone)]
pub struct Connection {
	session: web_transport::Session,
	listings: Listings,
}

impl Connection {
	pub fn new(session: web_transport::Session, listings: Listings) -> Self {
		Self { session, listings }
	}

	pub async fn run(self) -> anyhow::Result<()> {
		let (publisher, subscriber) = moq_transfork::Server::new(self.session.clone()).any().await?;

		let mut tasks = FuturesUnordered::new();

		if let Some(mut remote) = publisher {
			remote.announce(self.listings.broadcast()).await?;
			tasks.push(async move { remote.closed().await }.boxed());
		}

		if let Some(remote) = subscriber {
			tasks.push(Self::serve_publisher(self.clone(), remote).boxed());
		}

		// Return the first error
		tasks.select_next_some().await?;

		Ok(())
	}

	async fn serve_publisher(self, mut remote: Subscriber) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(announce) = remote.announced() => {
					let this = self.clone();
					tasks.push(this.serve_announce(announce));
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}

	#[tracing::instrument("serve", skip_all, err, fields(broadcast = announce.name))]
	async fn serve_announce(mut self, announce: BroadcastReader) -> anyhow::Result<()> {
		self.listings.register(&announce.name)?;
		announce.closed().await?;

		Ok(())
	}
}
