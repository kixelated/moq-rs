use std::{
	collections::{hash_map, HashMap},
	sync::{Arc, Mutex},
};

use anyhow::Context;

use moq_transport::{model::broadcast, session::Request, setup::Role};

#[derive(Clone)]
pub struct Session {
	broadcasts: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>,
}

impl Session {
	pub fn new(broadcasts: Arc<Mutex<HashMap<String, broadcast::Subscriber>>>) -> Self {
		Self { broadcasts }
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

		let path = request.uri().path().to_string();

		log::debug!("received WebTransport CONNECT: id={} path={}", id, path);

		// Accept the CONNECT request.
		let session = request
			.ok()
			.await
			.context("failed to respond to WebTransport request")?;

		log::debug!("accepted WebTransport CONNECT: id={} path={}", id, path);

		// Perform the MoQ handshake.
		let request = moq_transport::session::Server::accept(session)
			.await
			.context("failed to accept handshake")?;

		log::debug!("received MoQ SETUP: id={} role={:?}", id, request.role());

		let role = request.role();

		match role {
			Role::Publisher => self.serve_publisher(id, request, &path).await,
			Role::Subscriber => self.serve_subscriber(id, request, &path).await,
			Role::Both => request.reject(300),
		};

		log::debug!("closing connection: id={}", id);

		Ok(())
	}

	async fn serve_publisher(&mut self, id: usize, request: Request, path: &str) {
		log::info!("serving publisher: id={}, path={}", id, path);

		let (publisher, subscriber) = broadcast::new();

		match self.broadcasts.lock().unwrap().entry(path.to_string()) {
			hash_map::Entry::Occupied(_) => return request.reject(409),
			hash_map::Entry::Vacant(entry) => entry.insert(subscriber),
		};

		if let Err(err) = self.run_publisher(request, publisher).await {
			log::warn!("error serving pubisher: id={} path={} err={:?}", id, path, err);
		}

		self.broadcasts.lock().unwrap().remove(path);
	}

	async fn run_publisher(&mut self, request: Request, publisher: broadcast::Publisher) -> anyhow::Result<()> {
		let session = request.subscriber(publisher).await?;
		session.run().await?;
		Ok(())
	}

	async fn serve_subscriber(&mut self, id: usize, request: Request, path: &str) {
		log::info!("serving subscriber: id={} path={}", id, path);

		let broadcast = match self.broadcasts.lock().unwrap().get(path) {
			Some(broadcast) => broadcast.clone(),
			None => {
				return request.reject(404);
			}
		};

		if let Err(err) = self.run_subscriber(request, broadcast).await {
			log::warn!("error serving subscriber: id={} path={} err={:?}", id, path, err);
		}
	}

	async fn run_subscriber(&mut self, request: Request, broadcast: broadcast::Subscriber) -> anyhow::Result<()> {
		let session = request.publisher(broadcast).await?;
		session.run().await?;
		Ok(())
	}
}
