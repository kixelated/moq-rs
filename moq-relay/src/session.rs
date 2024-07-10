use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transfork::{Broadcast, RouterReader, SessionError};

use crate::Origins;

pub struct Session {
	session: web_transport::Session,
	incoming: Origins,
	outgoing: RouterReader<Broadcast>,
}

impl Session {
	pub fn new(session: web_transport::Session, incoming: Origins, outgoing: RouterReader<Broadcast>) -> Self {
		Self {
			session,
			incoming,
			outgoing,
		}
	}

	pub async fn run(self) -> Result<(), SessionError> {
		let (session, publisher, subscriber) = moq_transfork::Session::accept_any(self.session).await?;

		let mut tasks = FuturesUnordered::new();
		tasks.push(session.run().boxed());

		if let Some(mut publisher) = publisher {
			publisher.route(self.outgoing)
		}

		if let Some(subscriber) = subscriber {
			tasks.push(Self::run_producer(subscriber, self.incoming).boxed());
		}

		tasks.select_next_some().await
	}

	async fn run_producer(mut subscriber: moq_transfork::Subscriber, router: Origins) -> Result<(), SessionError> {
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
