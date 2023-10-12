use anyhow::Context;

use moq_transport::{cache::broadcast, session::Request, setup::Role, MoqError};

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
			Role::Publisher => self.serve_publisher(id, request, &path).await,
			Role::Subscriber => self.serve_subscriber(id, request, &path).await,
			Role::Both => {
				log::warn!("role both not supported: id={}", id);
				request.reject(300);
			}
		};

		log::debug!("closing connection: id={}", id);

		Ok(())
	}

	async fn serve_publisher(&mut self, id: usize, request: Request, path: &str) {
		log::info!("serving publisher: id={}, path={}", id, path);

		let broadcast = match self.origin.create_broadcast(path).await {
			Ok(broadcast) => broadcast,
			Err(err) => {
				log::warn!("error accepting publisher: id={} path={} err={:#?}", id, path, err);
				return request.reject(err.code());
			}
		};

		if let Err(err) = self.run_publisher(request, broadcast).await {
			log::warn!("error serving publisher: id={} path={} err={:#?}", id, path, err);
		}

		// TODO can we do this on drop? Otherwise we might miss it.
		self.origin.remove_broadcast(path).await.ok();
	}

	async fn run_publisher(&mut self, request: Request, publisher: broadcast::Publisher) -> anyhow::Result<()> {
		let session = request.subscriber(publisher).await?;
		session.run().await?;
		Ok(())
	}

	async fn serve_subscriber(&mut self, id: usize, request: Request, path: &str) {
		log::info!("serving subscriber: id={} path={}", id, path);

		let broadcast = self.origin.get_broadcast(path);

		if let Err(err) = self.run_subscriber(request, broadcast).await {
			log::warn!("error serving subscriber: id={} path={} err={:#?}", id, path, err);
		}
	}

	async fn run_subscriber(&mut self, request: Request, broadcast: broadcast::Subscriber) -> anyhow::Result<()> {
		let session = request.publisher(broadcast).await?;
		session.run().await?;
		Ok(())
	}
}
