use anyhow::Context;

use futures::{stream::FuturesUnordered, FutureExt, StreamExt};
use moq_transport::session::{Publisher, Subscriber};

use crate::Origin;

#[derive(Clone)]
pub struct Connection {
	origin: Origin,
}

impl Connection {
	pub fn new(origin: Origin) -> Self {
		Self { origin }
	}

	pub async fn run(self, conn: quinn::Connecting) -> anyhow::Result<()> {
		log::debug!("received QUIC handshake: ip={:?}", conn.remote_address());

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		log::debug!("established QUIC connection: ip={:?}", conn.remote_address(),);

		// Wait for the CONNECT request.
		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		// Strip any leading and trailing slashes to get the customer ID.
		let path = request.url().path().trim_matches('/').to_string();

		log::debug!("received WebTransport CONNECT: path={}", path);

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		let (session, publisher, subscriber) = moq_transport::Session::accept(session).await?;

		tokio::select! {
			res = session.run() => res?,
			res = Self::serve_publisher(publisher.unwrap(), self.origin.clone()), if publisher.is_some() => res?,
			res = Self::serve_subscriber(subscriber.unwrap(), self.origin), if subscriber.is_some() => res?,
		};

		Ok(())
	}

	async fn serve_publisher(mut publisher: Publisher, origin: Origin) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::info!("failed serving subscribe: err={}", err)
					}
				},
				res = publisher.subscribed() => {
					let subscribe = res?;
					log::info!("serving subscribe: namespace={} name={}", subscribe.namespace(), subscribe.name());
					tasks.push(origin.subscribe(subscribe).boxed());
				}
			};
		}
	}

	async fn serve_subscriber(mut subscriber: Subscriber, origin: Origin) -> anyhow::Result<()> {
		let mut tasks = FuturesUnordered::new();

		loop {
			tokio::select! {
				res = tasks.next(), if !tasks.is_empty() => {
					if let Err(err) = res.unwrap() {
						log::info!("failed serving announce: err={}", err)
					}
				},
				res = subscriber.announced() => {
					let announce = res?;
					log::info!("serving announce: namespace={}", announce.namespace());
					tasks.push(origin.announce(announce, subscriber.clone()));
				}
			};
		}
	}
}
