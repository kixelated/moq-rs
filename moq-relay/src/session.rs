use anyhow::Context;

use moq_transport::{model::broker, session, setup::Role, Error};

#[derive(Clone)]
pub struct Session {
	pub publisher: broker::Publisher,
	pub subscriber: broker::Subscriber,
}

impl Session {
	pub async fn run(&mut self, conn: quinn::Connecting) -> anyhow::Result<()> {
		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		// Wait for the CONNECT request.
		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		let path = request.uri().path().to_string();

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		// Perform the MoQ handshake.
		let request = moq_transport::session::Server::accept(session)
			.await
			.context("failed to accept handshake")?;

		let role = request.role();
		log::info!("received new session: path={} role={:?}", path, role);

		match role {
			Role::Publisher => {
				let subscriber = request.subscriber().await?;
				self.serve_publisher(subscriber).await?;
			}
			Role::Subscriber => {
				let publisher = request.publisher().await?;
				self.serve_subscriber(publisher).await?;
			}
			Role::Both => request.reject(300),
		};

		Ok(())
	}

	async fn serve_publisher(&mut self, session: session::Subscriber) -> Result<(), Error> {
		let mut announced = session.announced();

		log::info!("waiting for first announce");

		while let Some(broadcast) = announced.next_broadcast().await? {
			log::info!("received announce from publisher: {:?}", broadcast);
			self.publisher.insert_broadcast(broadcast)?;
		}

		Ok(())
	}

	async fn serve_subscriber(&mut self, mut session: session::Publisher) -> Result<(), Error> {
		while let Some(broadcast) = self.subscriber.next_broadcast().await? {
			log::info!("announcing broadcast to subscriber: {:?}", broadcast);
			session.announce(broadcast).await?;
		}

		Ok(())
	}
}
