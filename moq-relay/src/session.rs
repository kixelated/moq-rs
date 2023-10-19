use anyhow::Context;

use moq_transport::{session::Request, setup::Role, MoqError};

use crate::Origin;

#[derive(Clone)]
pub struct Session {
	origin: Origin,
}

impl Session {
	pub fn new(origin: Origin) -> Self {
		Self { origin }
	}

	pub async fn run(&mut self, conn: quinn::Connecting) -> anyhow::Result<()> {
		log::debug!("received QUIC handshake: ip={:?}", conn.remote_address());

		// Wait for the QUIC connection to be established.
		let conn = conn.await.context("failed to establish QUIC connection")?;

		log::debug!(
			"established QUIC connection: ip={:?} id={}",
			conn.remote_address(),
			conn.stable_id()
		);
		let id = conn.stable_id();

		// Wait for the CONNECT request.
		let request = webtransport_quinn::accept(conn)
			.await
			.context("failed to receive WebTransport request")?;

		// Strip any leading and trailing slashes to get the broadcast name.
		let path = request.url().path().trim_matches('/').to_string();

		log::debug!("received WebTransport CONNECT: id={} path={}", id, path);

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		// Perform the MoQ handshake.
		let request = moq_transport::session::Server::accept(session)
			.await
			.context("failed to accept handshake")?;

		log::debug!("received MoQ SETUP: id={} role={:?}", id, request.role());

		let role = request.role();

		match role {
			Role::Publisher => {
				if let Err(err) = self.serve_publisher(id, request, &path).await {
					log::warn!("error serving publisher: id={} path={} err={:#?}", id, path, err);
				}
			}
			Role::Subscriber => {
				if let Err(err) = self.serve_subscriber(id, request, &path).await {
					log::warn!("error serving subscriber: id={} path={} err={:#?}", id, path, err);
				}
			}
			Role::Both => {
				log::warn!("role both not supported: id={}", id);
				request.reject(300);
			}
		};

		log::debug!("closing connection: id={}", id);

		Ok(())
	}

	async fn serve_publisher(&mut self, id: usize, request: Request, path: &str) -> anyhow::Result<()> {
		log::info!("serving publisher: id={}, path={}", id, path);

		let mut origin = match self.origin.publish(path).await {
			Ok(origin) => origin,
			Err(err) => {
				request.reject(err.code());
				return Err(err.into());
			}
		};

		let session = request.subscriber(origin.broadcast.clone()).await?;

		tokio::select! {
			_ = session.run() => origin.close().await?,
			_ = origin.run() => (), // TODO send error to session
		};

		Ok(())
	}

	async fn serve_subscriber(&mut self, id: usize, request: Request, path: &str) -> anyhow::Result<()> {
		log::info!("serving subscriber: id={} path={}", id, path);

		let subscriber = self.origin.subscribe(path);

		let session = request.publisher(subscriber.broadcast.clone()).await?;
		session.run().await?;

		// Make sure this doesn't get dropped too early
		drop(subscriber);

		Ok(())
	}
}
