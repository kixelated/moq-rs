use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::prelude::*;

use crate::Origins;

pub struct Connection {
	session: moq_transfork::Server,
	incoming: Origins,
	outgoing: RouterReader<Broadcast>,
}

impl Connection {
	pub fn new(session: web_transport::Session, incoming: Origins, outgoing: RouterReader<Broadcast>) -> Self {
		Self {
			session: moq_transfork::Server::new(session),
			incoming,
			outgoing,
		}
	}

	pub async fn run(self) -> Result<(), SessionError> {
		let (publisher, subscriber) = self.session.any().await?;

		let mut tasks = FuturesUnordered::new();

		if let Some(mut publisher) = publisher {
			publisher.route(self.outgoing);
			tasks.push(async move { publisher.closed().await }.boxed());
		}

		if let Some(subscriber) = subscriber {
			tasks.push(Self::run_producer(subscriber, self.incoming).boxed());
		}

		tasks.select_next_some().await
	}

	async fn run_producer(mut subscriber: Subscriber, router: Origins) -> Result<(), SessionError> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				Some(broadcast) = subscriber.announced() => {
					// Announce that we're an origin for this broadcast
					let announce = router.announce(broadcast.clone());

					// Wait until the broadcast is closed to unannounce it
					tasks.push(async move {
						broadcast.closed().await.ok();
						drop(announce);
					})
				},
				_ = tasks.next(), if !tasks.is_empty() => {},
				else => return Ok(()),
			};
		}
	}
}
